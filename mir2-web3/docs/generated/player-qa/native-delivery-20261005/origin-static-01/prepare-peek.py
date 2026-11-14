from pathlib import Path

root=Path(__file__).absolute().parent
source=(root/"remote-preflight.py").read_text().split('if __name__ == "__main__":')[0]
body='''
stage=Path("/srv/.mir2-origin-r17-acceleration-20261005-01")
proofs=sorted(stage.glob("SIGNED-BUNDLE-*.json"))
parts=[]
directory=stage/"generated/delivery"
if directory.exists():
    for path in sorted(directory.iterdir()):
        info=path.lstat()
        require(stat.S_ISREG(info.st_mode) and info.st_nlink==1 and info.st_uid==0,"unsafe private output")
        parts.append({"name":path.name,"size":info.st_size})
memory={line.split(":",1)[0]:line.split(":",1)[1].strip() for line in Path("/proc/meminfo").read_text().splitlines()}
result={"passed":True,"privateGenerationProofCount":len(proofs),"outputs":parts,
        "firstBundleProof":json.loads(proofs[0].read_bytes())["archive"] if proofs else None,
        "generationCompleted":(stage/"GENERATION-RESULT.json").exists(),
        "memoryAvailableBytes":int(memory["MemAvailable"].split()[0])*1024,
        "loadAverage":list(os.getloadavg()),"health":[health(7110),health(7210)],"services":services(),
        "timestampUnix":int(time.time()),"publicDescriptorExists":Path("/srv/mir2-client-updates/releases/game-WN-CANDIDATE-20261004-invited-17/DELIVERY.json").exists()}
print(json.dumps(result,sort_keys=True,indent=2))
'''
with (root/"peek-generation.command").open("x",newline="\n") as stream:
    stream.write("sudo -n python3 -B - <<'MIR2_READ_PRIVATE_GENERATION_EOF'\n"+source+body+"\nMIR2_READ_PRIVATE_GENERATION_EOF\n")
