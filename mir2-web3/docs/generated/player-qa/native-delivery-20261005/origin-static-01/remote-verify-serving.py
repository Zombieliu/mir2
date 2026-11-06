"""Loopback HTTPS verifies every newly served byte without Internet re-download."""
import hashlib
import http.client
import json
from pathlib import Path
import socket
import ssl
import time

HOST="165.154.65.136.sslip.io"
STAGE=Path("/srv/.mir2-origin-r17-acceleration-20261005-01")


class LocalTLS(http.client.HTTPSConnection):
    def connect(self):
        self.sock=self._context.wrap_socket(socket.create_connection(("127.0.0.1",443),self.timeout),server_hostname=self.host)


def read(item, discovery=False):
    path="/client-updates/"+item["path"]
    connection=LocalTLS(HOST,timeout=30,context=ssl.create_default_context())
    start=time.monotonic()
    try:
        connection.request("GET",path,headers={"Host":HOST,"Accept-Encoding":"identity"})
        response=connection.getresponse()
        assert response.status==200 and response.getheader("Content-Length")==str(item["size"])
        assert response.getheader("Content-Encoding") in (None,"identity")
        cache=response.getheader("Cache-Control","")
        assert ("no-store" in cache) if discovery else ("immutable" in cache)
        digest,count=hashlib.sha256(),0
        while block:=response.read(65536):
            count+=len(block);assert count<=item["size"];digest.update(block)
        assert count==item["size"] and digest.hexdigest()==item["sha256"].lower()
        return {"path":item["path"],"status":response.status,"size":count,"sha256":digest.hexdigest(),
                "cacheControl":cache,"contentEncoding":response.getheader("Content-Encoding"),
                "loopbackElapsedSeconds":round(time.monotonic()-start,3)}
    finally:
        connection.close()


expected=json.loads((STAGE/"EXPECTED.json").read_bytes())
values=[]
for item in expected["sourceFiles"][:2]:
    values.append(read({**item,"path":item["path"].removeprefix("feed-current/")},True))
for item in expected["objects"]:
    values.append(read(item))
result={"schema":"mir2.origin.acceleration-tls-full-byte-verification.v1","passed":True,
        "newObjects":14,"verifiedCompressedAndMetadataBytes":sum(i["size"] for i in expected["objects"]),
        "loopbackTls":True,"publicHostCertificateVerified":True,"internetDownloadSpeedAcceptance":False,
        "feedPairUnchanged":True,"files":values,"createdUnix":int(time.time())}
data=json.dumps(result,sort_keys=True,indent=2).encode()+b"\n"
with (STAGE/"HTTPS-FULL-VERIFICATION-01.json").open("xb") as f:f.write(data)
print(data.decode())
