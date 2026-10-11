"""Read-only production-key checks; credentials and provider objects never print."""
import argparse
import json
import pathlib
import re
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone

API_VERSION = '2026-09-30.endive'
MAX_BYTES = 262144


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        return None


def load_key(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_nlink != 1:
        raise ValueError('ordinary-private-key-file-required')
    if path.stat().st_size > 16384:
        raise ValueError('key-file-too-large')
    values = {}
    for raw in path.read_text(encoding='utf-8-sig').splitlines():
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        name, sep, value = line.partition('=')
        if not sep or name.strip() in values:
            raise ValueError('invalid-or-duplicate-key-assignment')
        values[name.strip()] = value.strip().strip('"').strip("'")
    key = values.get('MIR2_STRIPE_SECRET_KEY', '')
    if key.startswith(('sk_test_', 'rk_test_')):
        raise ValueError('test-key-cannot-enable-real-payments')
    if len(key) < 24 or re.fullmatch(r'(sk|rk)_live_[A-Za-z0-9]+', key) is None:
        raise ValueError('live-key-required')
    return key


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--key-file', type=pathlib.Path, required=True, help='Private credential file outside the repository')
    parser.add_argument('--receipt', type=pathlib.Path)
    args = parser.parse_args()
    report = {'schema': 'mir2.stripe-live.provider-readiness.v1',
              'checkedAtUtc': datetime.now(timezone.utc).isoformat(),
              'apiVersion': API_VERSION, 'ready': False, 'checks': [],
              'chargesCreated': False, 'providerObjectsChanged': False}
    try:
        key = load_key(args.key_file)
    except ValueError as error:
        report['blocked'] = str(error)
    except Exception as error:
        report['blocked'] = type(error).__name__
    else:
        report['credentialMode'] = 'live'
        opener = urllib.request.build_opener(NoRedirect())
        for endpoint in ('account', 'checkout/sessions?limit=1', 'payment_intents?limit=1', 'charges?limit=1', 'webhook_endpoints?limit=1'):
            request = urllib.request.Request('https://api.stripe.com/v1/'+endpoint,
                headers={'Authorization': 'Bearer '+key, 'Stripe-Version': API_VERSION})
            check = {'endpoint': endpoint}
            try:
                with opener.open(request, timeout=20) as response:
                    raw = response.read(MAX_BYTES+1)
                    if len(raw) > MAX_BYTES:
                        raise ValueError('provider-response-too-large')
                    value = json.loads(raw)
                    check['httpStatus'] = response.status
                if endpoint == 'account':
                    check.update(chargesEnabled=value.get('charges_enabled'),
                        payoutsEnabled=value.get('payouts_enabled'),
                        detailsSubmitted=value.get('details_submitted'),
                        cardPaymentsCapability=value.get('capabilities', {}).get('card_payments'),
                        currentlyDueCount=len(value.get('requirements', {}).get('currently_due', [])))
            except urllib.error.HTTPError as error:
                check['httpStatus'] = error.code
            except Exception as error:
                check['errorType'] = type(error).__name__
            report['checks'].append(check)
        account = report['checks'][0]
        report['ready'] = (all(item.get('httpStatus') == 200 for item in report['checks'])
                           and account.get('chargesEnabled') is True
                           and account.get('cardPaymentsCapability') == 'active')
        if not report['ready']:
            report['blocked'] = 'account-or-api-permissions-not-ready'
        key = None
    if args.receipt:
        with args.receipt.open('x', encoding='utf-8') as stream:
            json.dump(report, stream, indent=2)
            stream.write('\n')
    print(json.dumps(report, ensure_ascii=True))
    return 0 if report['ready'] else 2


if __name__ == '__main__':
    sys.exit(main())
