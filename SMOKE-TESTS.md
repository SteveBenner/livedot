# Smoke tests

`polispec smoke record` writes this file; edit nothing by hand.

| Subject | Verdict | Recorded | ID | Summary |
| --- | --- | --- | --- | --- |
| dot-size-hide-healthy | partial | 2026-11-23T02:24:24Z | smk_0e773d0713b2 | Claim: dot_size scales the tray dot and hide_healthy drops green dots, both persisted into config.yml and published i... |
| icon-size-hide-healthy | partial | 2026-11-23T03:39:19Z | smk_4e51bfb7922d | Claim: icon_size replaces dot_size and leaves status dots at 0.11.0 size; hide_healthy keeps every service and drops ... |

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
- id: smk_4e51bfb7922d
  recorded_at: '2026-11-23T03:39:19Z'
  proves:
    feature: icon-size-hide-healthy
  verdict: partial
  summary: 'Claim: icon_size replaces dot_size and leaves status dots at 0.11.0 size;
    hide_healthy keeps every service and drops only the green badge from a logo. Rung:
    real daemon and tray on a private session bus. Seen: range and leftover dot_size
    refused; iconsize 150 rewrote one line; SetDotSize unknown; with hide_healthy
    true all three items kept, green logo unbadged (new hash), green plain dot and
    red badged logo unchanged; plain dot disc 2058 px at icon_size 150. Unproven:
    the GNOME extension''s scaled logo, hidden badge and Icon size tile were not run
    in a Shell.'
  steps: []
  evidence:
  - path: check.txt
    sha256: 6d55b8f2d0a242dd0f6f5f0306f9d4a9f9f8b4eb0069a312a3c1dc11871bdfe9
    bytes: 1065
    kind: file
  - path: run.txt
    sha256: 680d2137c5e87a7a1cd304cd5e559be3a23dc4749a91ece6b90d82dff6298eba
    bytes: 1947
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
    commit: 4e1a5f06106dd5c36cc5aa585cfed69c2cb4d392
    branch: main
    dirty: true
    project:
    env:
    policy_digest:
  prev: 7d02615fa182e528abfc55b3810adaa51d476305c4c3e6b8574fabba699761e1
  digest: 2c14debccf0a60038d22eca55a5519357b97a2b71b03d7e2c2be5fc0e7bccb56
```
