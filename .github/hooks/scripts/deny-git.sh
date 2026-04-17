#!/bin/sh

INPUT=$(cat)

if [ -z "$INPUT" ]; then
  exit 0
fi

TOOL_NAME=$(printf '%s' "$INPUT" | sed -n 's/.*"toolName"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
if [ "$TOOL_NAME" != "bash" ] && [ "$TOOL_NAME" != "powershell" ] && [ "$TOOL_NAME" != "shell" ]; then
  exit 0
fi

if ! printf '%s\n' "$INPUT" | grep -Eq '"toolArgs"[[:space:]]*:[[:space:]]*"[^"]*(^|[^[:alnum:]_.-])git(\\.exe)?([^[:alnum:]_.-]|$)[^"]*"'; then
  exit 0
fi

printf '%s\n' '{"permissionDecision":"deny","permissionDecisionReason":"Copilot CLI may not run git commands in this repository."}'
