cd ~/test/qwen38

# -----------------------------------------------------------------
pkill -9 opencode ; pkill -9 mcode; pkill -9 python; pkill -9 llama-server
rm /home/lorenzo/.logs/ropero/qwen38.log
ropero qwen38 --background
sleep 20
mkdir -p ~/test/qwen38/prueba1
cd ~/test/qwen38/prueba1
opencode run "$(</home/lorenzo/workspace/github/lasaca/ropero/tests/prueba1.md)" --agent build --model "marichocho/Puertourraco-256k" --format json  > prueba1.json
session1=$(cat prueba1.json | jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' | sort | uniq )
opencode session export $session1 > sesion-prueba1.md
cp /home/lorenzo/.logs/ropero/qwen38.log log-llama-sesion-prueba1.txt
# -----------------------------------------------------------------


# -----------------------------------------------------------------
pkill -9 opencode ; pkill -9 mcode; pkill -9 python; pkill -9 llama-server
rm /home/lorenzo/.logs/ropero/qwen38.log
ropero qwen38 --background
sleep 20
mkdir -p ~/test/qwen38/prueba2
cd ~/test/qwen38/prueba2
opencode run "$(</home/lorenzo/workspace/github/lasaca/ropero/tests/prueba2.md)" --agent build --model "marichocho/Puertourraco-256k" --format json  > prueba2.json
session2=$(cat prueba2.json | jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' | sort | uniq )
opencode session export $session2 > sesion-prueba2.md
cp /home/lorenzo/.logs/ropero/qwen38.log log-llama-sesion-prueba2.txt
# -----------------------------------------------------------------


# -----------------------------------------------------------------
pkill -9 opencode ; pkill -9 mcode; pkill -9 python; pkill -9 llama-server
rm /home/lorenzo/.logs/ropero/qwen38.log
ropero qwen38 --background
sleep 20
mkdir -p ~/test/qwen38/prueba3
cd ~/test/qwen38/prueba3
opencode run "$(</home/lorenzo/workspace/github/lasaca/ropero/tests/prueba3.md)" --agent build --model "marichocho/Puertourraco-256k" --format json  > prueba3.json
session3=$(cat prueba3.json | jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' | sort | uniq )
opencode session export $session3 > sesion-prueba3.md
cp /home/lorenzo/.logs/ropero/qwen38.log log-llama-sesion-prueba3.txt
# -----------------------------------------------------------------


# -----------------------------------------------------------------
pkill -9 opencode ; pkill -9 mcode; pkill -9 python; pkill -9 llama-server
rm /home/lorenzo/.logs/ropero/qwen38.log
ropero qwen38 --background
sleep 20
mkdir -p ~/test/qwen38/prueba_larga
cd ~/test/qwen38/prueba_larga
opencode run "$(</home/lorenzo/workspace/github/lasaca/ropero/tests/prueba_larga.md)" --agent build --model "marichocho/Puertourraco-256k" --format json  > prueba_larga.json
session_larga=$(cat prueba_larga.json | jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' | sort | uniq )
opencode session export $session_larga > sesion-prueba_larga.md
cp /home/lorenzo/.logs/ropero/qwen38.log log-llama-sesion-prueba_larga.txt
# -----------------------------------------------------------------


# -----------------------------------------------------------------
pkill -9 opencode ; pkill -9 mcode; pkill -9 python; pkill -9 llama-server
rm /home/lorenzo/.logs/ropero/qwen38.log
ropero qwen38 --background
sleep 20
mkdir -p ~/test/qwen38/prueba_gorda
cd ~/test/qwen38/prueba_gorda
opencode run "$(</home/lorenzo/workspace/github/lasaca/ropero/tests/prueba_gorda.md)" --agent build --model "marichocho/Puertourraco-256k" --format json  > prueba_gorda.json
session_gorda=$(cat prueba_gorda.json | jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' | sort | uniq )
opencode session export $session_gorda > sesion-prueba_gorda.md
cp /home/lorenzo/.logs/ropero/qwen38.log log-llama-sesion-prueba_gorda.txt
# -----------------------------------------------------------------
