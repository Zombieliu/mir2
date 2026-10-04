"""Bounded direct verified-TLS Internet verification, not a full-speed claim."""
import hashlib
import http.client
import json
from pathlib import Path
import ssl
import time

root=Path(__file__).absolute().parent
expected=json.loads((root/"EXPECTED-02.json").read_bytes())
sources=json.loads((root.parent/"origin-download-compatibility-plan-01/ORIGIN-ADDITIONS.json").read_bytes())["objects"]
host="165.154.65.136.sslip.io"
prefix="/client-updates/"
records=[]


def request(method,path,expected_size=None,expected_sha=None,byte_range=None):
    connection=http.client.HTTPSConnection(host,timeout=30,context=ssl.create_default_context())
    start=time.monotonic()
    headers={"Accept-Encoding":"identity"}
    if byte_range is not None:
        headers["Range"]="bytes=%d-%d"%byte_range
    try:
        connection.request(method,prefix+path,headers=headers)
        response=connection.getresponse()
        assert response.status==(206 if byte_range is not None else 200),(path,response.status)
        assert response.getheader("Content-Encoding") in (None,"identity")
        assert response.getheader("Content-Length")==str(expected_size)
        cache=response.getheader("Cache-Control","")
        assert "no-store" in cache if path in ("latest.json","latest.p7s") else "immutable" in cache
        if byte_range is not None:
            complete=next(item["size"] for item in expected["objects"] if item["path"]==path)
            assert response.getheader("Content-Range")=="bytes %d-%d/%d"%(*byte_range,complete)
        count,digest=0,hashlib.sha256()
        while block:=response.read(65536):
            count+=len(block);assert count<=expected_size;digest.update(block)
        if method=="GET":
            assert count==expected_size and digest.hexdigest()==expected_sha.lower(),path
        else:
            assert count==0
        elapsed=time.monotonic()-start
        record={"method":method,"path":path,"status":response.status,
                "contentLength":expected_size,"bytesRead":count,"cacheControl":cache,
                "contentEncoding":response.getheader("Content-Encoding"),
                "contentRange":response.getheader("Content-Range"),
                "elapsedSeconds":round(elapsed,6),"sha256":digest.hexdigest() if method=="GET" else None,
                "MiBPerSecond":round(count/1024**2/elapsed,6) if count else None}
        records.append(record)
        return record
    finally:
        connection.close()


for item in expected["sourceFiles"][:2]:
    request("GET",item["path"].removeprefix("feed-current/"),item["size"],item["sha256"])
for item in expected["objects"]:
    request("HEAD",item["path"],item["size"])
print(json.dumps({"stage":"public-headers-verified","objects":14}),flush=True)
for suffix in ("DELIVERY.json","DELIVERY.p7s"):
    item=next(item for item in expected["objects"] if item["path"].endswith("/"+suffix))
    value=request("GET",item["path"],item["size"],item["sha256"])
    print(json.dumps({"stage":"public-signed-metadata-verified","path":suffix,"seconds":value["elapsedSeconds"]}),flush=True)
descriptor=json.loads((root.parent/"r17-download-acceleration-02/DELIVERY.json").read_bytes())
first=descriptor["bundles"][0]["archive"]
path=expected["candidateDirectory"]+"/"+first["path"]
source=Path(next(item["source"] for item in sources if item["path"]==path))
sample=1024**2
for offset in (0,first["size"]-sample):
    with source.open("rb") as file:
        file.seek(offset);data=file.read(sample);assert len(data)==sample
    value=request("GET",path,sample,hashlib.sha256(data).hexdigest(),(offset,offset+sample-1))
    print(json.dumps({"stage":"public-artifact-sample-verified","offset":offset,"seconds":value["elapsedSeconds"]}),flush=True)
result={"schema":"mir2.origin.bounded-public-download-verification.v1","passed":True,
        "createdUnix":int(time.time()),"feedAndSignedMetadataMatch":True,"immutableObjectHeads":14,
        "sampledArtifactBytes":2*sample,"fullInternetArtifactsDownloaded":False,
        "TLSCertificateVerified":True,"proxyUsed":False,"allRequests":records,
        "sourceBandwidthSolved":False,"CDNPublished":False}
with (root/"PUBLIC-VERIFICATION-01.json").open("x",newline="\n") as out:
    json.dump(result,out,sort_keys=True,indent=2)
print(json.dumps({"passed":True,"totalInternetBytes":sum(item["bytesRead"] for item in records)}))
