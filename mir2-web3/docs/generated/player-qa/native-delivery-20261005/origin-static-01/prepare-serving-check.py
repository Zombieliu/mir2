from pathlib import Path

root=Path(__file__).absolute().parent
script=(root/"remote-verify-serving.py").read_text()
with (root/"verify-serving-01.command").open("x",newline="\n") as f:
    f.write("sudo -n nice -n 10 python3 -B - <<'MIR2_VERIFY_ORIGIN_TLS_EOF'\n"+script+"\nMIR2_VERIFY_ORIGIN_TLS_EOF\n")
