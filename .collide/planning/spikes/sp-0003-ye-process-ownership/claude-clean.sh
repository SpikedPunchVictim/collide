#!/bin/sh
# Run `claude` with every CLAUDE* variable of the calling session removed,
# so a spike started from inside Claude Code does not look like a nested session.
for v in $(env | sed -n 's/^\(CLAUDE[A-Z0-9_]*\)=.*/\1/p'); do unset "$v"; done
exec claude "$@"
