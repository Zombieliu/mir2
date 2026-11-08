"""Probe existing credentials without writing any R2 object or logging values."""
import json,os,re,urllib.error,urllib.parse,urllib.request

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs): return None

opener=urllib.request.build_opener(NoRedirect())
account=os.environ['CLOUDFLARE_ACCOUNT_ID'].strip()
token=os.environ['CLOUDFLARE_API_TOKEN']
bucket=(os.environ.get('MIR2_R2_ASSET_BUCKET') or os.environ.get('MIR2_R2_BUCKET') or 'mir2-web3-assets').strip()
requests=[
 ('token_verification',urllib.request.Request('https://api.cloudflare.com/client/v4/user/tokens/verify',headers={'Authorization':'Bearer '+token})),
 ('r2_bucket_read',urllib.request.Request(f'https://api.cloudflare.com/client/v4/accounts/{account}/r2/buckets/{urllib.parse.quote(bucket,safe="")}',headers={'Authorization':'Bearer '+token})),
]
url=urllib.parse.urlsplit(os.environ.get('MIR2_R2_UPLOAD_WORKER_URL',''))
secret=os.environ.get('MIR2_R2_UPLOAD_SECRET','')
allowed_host=url.hostname=='assets.mir2.obelisk.build' or bool(re.fullmatch(r'mir2-r2-bulk-upload\.[a-z0-9-]+\.workers\.dev',url.hostname or ''))
if url.scheme!='https' or not allowed_host or url.path!='/upload' or url.username or url.password or url.port or not secret:
    raise SystemExit('Existing upload service binding did not match reviewed origin/path')
probe_url=urllib.parse.urlunsplit((url.scheme,url.netloc,url.path,urllib.parse.urlencode({'key':'invalid-probe-key-no-write'}),''))
requests.append(('existing_worker_auth_and_key_rejection',urllib.request.Request(probe_url,data=b'',method='PUT',headers={'Authorization':'Bearer '+secret,'Content-Type':'application/octet-stream'})))
for label,req in requests:
    try:
        with opener.open(req,timeout=30) as r:
            status=r.status;body=r.read(65536)
    except urllib.error.HTTPError as e:
        status=e.code;body=e.read(65536)
    try:
        data=json.loads(body)
        safe={'error':data.get('error'),'codes':[v.get('code') for v in data.get('errors',[])],'success':data.get('success')}
    except (ValueError,AttributeError):safe={'json':False}
    print(json.dumps({'probe':label,'httpStatus':status,**safe}),flush=True)
    if label=='existing_worker_auth_and_key_rejection' and not(status==400 and safe.get('error')=='invalid_key'):
        raise SystemExit('Existing upload Worker authentication was not confirmed')
