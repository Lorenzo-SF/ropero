#!/usr/bin/env python3
"""Lanza ropero en un pty y le manda un Ctrl-C de verdad (SIGINT al grupo
de procesos del foreground). Es lo que hace la terminal; lanzar el script con
& NO sirve, porque bash ignora SIGINT en los jobs de background.

Uso: ctrlc.py <salida.txt> -- <cmd...>
Imprime el código de salida del hijo al final (precedido de "EXIT=")."""
import os
import pty
import sys
import time

argv = sys.argv[1:]
outfile = argv[0]
sep = argv.index("--")
cmd = argv[sep + 1:]

pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm"
    os.execvp(cmd[0], cmd)

out = open(outfile, "wb")
sent = False
start = time.time()
while True:
    now = time.time() - start
    if now > 16:
        break
    try:
        data = os.read(fd, 65536)
    except OSError:
        break
    if not data:
        break
    out.write(data)
    out.flush()
    if not sent and now > 4:
        os.write(fd, b"\x03")   # Ctrl-C
        sent = True
out.close()

try:
    _, status = os.waitpid(pid, os.WNOHANG)
    if _ == 0:
        os.kill(pid, 9)
        _, status = os.waitpid(pid, 0)
        sys.stdout.write("EXIT=matado (seguia vivo tras el Ctrl-C)\n")
    else:
        sys.stdout.write("EXIT=%d (signal=%d)\n" % (status >> 8, status & 0x7F))
except ChildProcessError:
    sys.stdout.write("EXIT=?\n")
