#!/bin/sh
# Run `claude` with every CLAUDE* variable of the calling session removed,
# so a spike started from inside Claude Code does not look like a nested session.
# (Copied from S1. S9 name: names listed in S9_PASS, space-separated, are kept, so a script
# can pass one deliberate CLAUDE_* setting such as CLAUDE_CODE_DISABLE_AUTO_MEMORY.)
for v in $(env | sed -n 's/^\(CLAUDE[A-Z0-9_]*\)=.*/\1/p'); do
  case " ${S9_PASS:-} " in
    *" $v "*) ;;
    *) unset "$v" ;;
  esac
done
unset S9_PASS
exec claude "$@"
