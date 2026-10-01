set -eu
sudo -n sha256sum /etc/caddy/Caddyfile
df -h /srv
for port in 7110 7210; do
 curl -fsS --max-time 15 http://127.0.0.1:$port/health | python3 -c 'import json,sys; h=json.load(sys.stdin); print(json.dumps({k:h.get(k) for k in ["ok","http","ws","revision","capacity"]}))'
 printf '
'
done
