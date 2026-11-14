#!/usr/bin/env bash
# Server hardening for team-06 (#11). Safe to re-run.
#
#   ssh team-06 'sudo bash -s' < deploy/harden.sh
#
# Keep an SSH session open while it runs, and check that a fresh `ssh team-06` still works after.
# Docker-published ports bypass ufw, so compose.yml must publish only Caddy's 80/443.
set -euo pipefail

# 1. The planted debug console (python http.server on 0.0.0.0:8888). It has Restart=always,
#    so killing the process isn't enough: disable the unit.
if systemctl list-unit-files debug-console.service --no-legend | grep -q debug-console; then
  systemctl disable --now debug-console
fi

# 2. Firewall: deny incoming except SSH, HTTP, HTTPS (and HTTP/3 over UDP).
sed -i 's/^IPV6=.*/IPV6=yes/' /etc/default/ufw
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 443/udp
ufw --force enable
ufw status verbose
