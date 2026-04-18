# Shiftwrangler

> Round up your agents. Rest easy. Ride out at dawn.

`swctl` is the CLI for Shiftwrangler — an agent session lifecycle manager that pauses, sleeps, wakes, and resumes your AI agent sessions automatically.

## Quick start

```sh
swctl schedule set --suspend-at 23:00 --wake-at 08:00
swctl daemon start
```

## Commands

```
swctl daemon   start | stop | status
swctl session  list | add | remove
swctl schedule show | set | clear
swctl suspend  now | resume
```
