"""Freeze an explicit inventory of this new public execution folder; read-only elsewhere."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json

ROOT = Path(__file__).resolve().parent
INPUTS = ROOT.parent / 'dense-network-r18-inputs-01'
V6 = ROOT.parent.parent / '20261004-native-r17' / 'dense-network-qa-v6'
OUT = ROOT / 'FINAL-PUBLIC-INVENTORY-01.json'

def sha(p):
    with p.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def doc(p):
    return json.loads(p.read_text(encoding='utf-8'))

def check_frozen(folder, expected):
    p = folder / 'FROZEN-INPUTS.json'
    assert sha(p) == expected
    for entry in doc(p)['files']:
        q = folder / entry['relativePath']
        assert q.is_file() and not q.is_symlink() and q.stat().st_size == entry['bytes'] and sha(q) == entry['sha256']
    return {'manifestPath': str(p), 'manifestSha256': expected, 'fileCountVerified': len(doc(p)['files']), 'allOriginalBytesUnchanged': True}

def main():
    r18 = check_frozen(INPUTS, 'b7ac4b4f33cc746248a6e6c0f2d9d97a5822fc4be5e953e5f1e08aad34dfc0c0')
    v6 = check_frozen(V6, '091bebe3a713b3788a52e9b5bc22b1cca25b029fe24794c81a32f6cfa8386366')
    assert doc(ROOT/'INDEPENDENT-RAW-REVIEW-02.json')['verificationComplete'] is True
    assert doc(ROOT/'OFFLINE-CLASSIFIER-REVIEW-01.json')['verificationComplete'] is True
    assert doc(ROOT/'DOWNLOADED-RECEIPTS-01.json')['privateStoresKeysOrRawGatewayLogsIncluded'] is False
    files = []
    for p in sorted(ROOT.rglob('*')):
        if p.is_dir():
            assert not p.is_symlink()
            continue
        if p == OUT:
            raise RuntimeError('Final receipt already exists: do not rewrite the frozen result')
        relative = p.relative_to(ROOT).as_posix()
        assert p.is_file() and not p.is_symlink() and p.stat().st_nlink == 1
        assert all(not part.startswith('private-') and part != '__pycache__' for part in Path(relative).parts)
        files.append({'relativePath': relative, 'bytes': p.stat().st_size, 'sha256': sha(p)})
    assert sum(f['relativePath'].startswith('downloaded-public/') for f in files) == 45
    receipt = {'schema': 'mir2.r18-single-execution-final-public-inventory.v1', 'createdUtc': datetime.now(timezone.utc).isoformat(),
        'sourceRevision': '321316b791120a6fe549e4006623e63333a5543f',
        'candidateId': 'WN-CANDIDATE-20261004-invited-18',
        'binarySha256': '4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8',
        'binaryBytes': 79856480, 'ciRun': 37204097306, 'ciArtifact': 11304620835,
        'r18InputsVerification': r18, 'originalV6InputsVerification': v6,
        'verificationComplete': True, 'executionResultFrozen': True,
        'singleExecution': True, 'noGatewayRetry': True, 'noAdditionalExecutionAuthorized': True,
        'localPublicFileCount': len(files), 'downloadedPublicFileCount': 45, 'allExtractedPublicFilesIndividuallyVerified': True,
        'privateStoreKeyOrRawGatewayLogContentIncluded': False, 'privateOriginalsRemainRemote': True,
        'reviewerBoundaryAssertionFailureRetained': 'INDEPENDENT-RAW-REVIEW-01.log',
        'independentCorrectedReview': 'INDEPENDENT-RAW-REVIEW-02.json',
        'offlineClassifierReview': 'OFFLINE-CLASSIFIER-REVIEW-01.json',
        'originalReportedResultsAndAllRawReceiptsUnchanged': True,
        'sevenMonsterMechanicsAccepted': False, 'fullP1Accepted': False, 'nativeInputAccepted': False,
        'nativeVisualAccepted': False, 'humanAccepted': False, 'ordinaryProgressionAccepted': False,
        'loadAccepted': False, 'capacityAccepted': False, 'files': files}
    with OUT.open('x', encoding='utf-8', newline='\n') as f:
        json.dump(receipt, f, indent=2, ensure_ascii=False); f.write('\n')
    print(json.dumps({'path':str(OUT),'sha256':sha(OUT),'localPublicFiles':len(files),'downloadedPublicFiles':45,'originalInputsUnchanged':True}))

if __name__ == '__main__':
    main()
