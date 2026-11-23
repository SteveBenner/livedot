# Smoke tests

`polispec smoke record` writes this file; edit nothing by hand.

| Subject | Verdict | Recorded | ID | Summary |
| --- | --- | --- | --- | --- |
| dot-size-hide-healthy | partial | 2026-11-23T02:24:24Z | smk_0e773d0713b2 | Claim: dot_size scales the tray dot and hide_healthy drops green dots, both persisted into config.yml and published i... |

## Ledger

```yaml
schema: polispec.smoke/v1
entries:
- id: smk_0e773d0713b2
  recorded_at: '2026-11-23T02:24:24Z'
  proves:
    feature: dot-size-hide-healthy
  verdict: partial
  summary: 'Claim: dot_size scales the tray dot and hide_healthy drops green dots,
    both persisted into config.yml and published in GetState. Rung: real daemon and
    tray on a private session bus with scratch config; read config diffs, StatusNotifierItem
    ids and 64px pixmap coverage (disc area 514, 2058, 3216 px at 50, 100, 150). Unproven:
    GNOME extension tile, scaled top-bar dots and Logo badge were not run in a Shell;
    nothing installed.'
  steps: []
  evidence:
  - path: run.txt
    sha256: cc1e763de9479e4eb27771ecd0dcee07df9539bfec96f9fc86708eb41f274587
    bytes: 941
    kind: file
  - path: run2.txt
    sha256: '0097eb9cadf3fbbcbd342ff8389a41c56fb3b9b15c91ca83da3a6b6cc65150c7'
    bytes: 272
    kind: file
  runner:
    kind: agent
    harness: claude-code
    harness_version: 2.1.330
    model:
    effort:
  runtime:
    os:
      id: ubuntu
      name: Ubuntu
      version: '26.04'
    kernel: 7.0.0-42-generic
    arch: x86_64
    platform: x86_64-linux
    container: false
    wsl: false
    tz: PST
    locale: en_US.UTF-8
    ruby:
      engine: ruby
      version: 4.0.6
      platform: x86_64-linux
    polispec:
      version: 0.59.2
      edition: standard
      commit: 25aea6fd344f87e0598533e8293776af685b8b32
      dirty: false
      install: pinned
    rplugin: 1.20.6
    enforce: enforce
  target:
    commit: 1d222b884d7d395bc7494077199f341f9acab041
    branch: main
    dirty: true
    project:
    env:
    policy_digest:
  prev:
  digest: 7d02615fa182e528abfc55b3810adaa51d476305c4c3e6b8574fabba699761e1
```
