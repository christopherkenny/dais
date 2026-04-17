$raw = [Console]::In.ReadToEnd()

if ([string]::IsNullOrWhiteSpace($raw)) {
    exit 0
}

try {
    $event = $raw | ConvertFrom-Json
} catch {
    exit 0
}

$toolName = [string]$event.toolName
if ($toolName -notin @("bash", "powershell", "shell")) {
    exit 0
}

$toolArgsRaw = [string]$event.toolArgs
if ([string]::IsNullOrWhiteSpace($toolArgsRaw)) {
    exit 0
}

$gitPattern = '(^|[^\w.-])git(\.exe)?([^\w.-]|$)'
if ($toolArgsRaw -match $gitPattern) {
    @{
        permissionDecision = "deny"
        permissionDecisionReason = "Copilot CLI may not run git commands in this repository."
    } | ConvertTo-Json -Compress
}
