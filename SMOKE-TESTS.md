# Smoke tests

`polispec smoke record` writes this file; edit nothing by hand.

| Subject | Verdict | Recorded | ID | Summary |
| --- | --- | --- | --- | --- |
| dot-size-hide-healthy | partial | 2026-11-23T02:24:24Z | smk_0e773d0713b2 | Claim: dot_size scales the tray dot and hide_healthy drops green dots, both persisted into config.yml and published i... |
| icon-size-hide-healthy | partial | 2026-11-23T03:39:19Z | smk_4e51bfb7922d | Claim: icon_size replaces dot_size and leaves status dots at 0.11.0 size; hide_healthy keeps every service and drops ... |
| gnome-switch-rows | blocked | 2026-11-23T08:20:52Z | smk_1ecdb9467679 | Switch rows in the Dots, Hide healthy and switch tiles sent a second, opposite call on GNOME 50; fix installed (exten... |
| panel-position | partial | 2026-11-23T08:24:59Z | smk_b05d114a62d3 | Daemon 0.13.0 on a private bus: position defaults right, set center and left via CLI, GetState carries panel_position... |

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
- id: smk_1ecdb9467679
  recorded_at: '2026-11-23T08:20:52Z'
  proves:
    feature: gnome-switch-rows
  verdict: blocked
  summary: 'Switch rows in the Dots, Hide healthy and switch tiles sent a second,
    opposite call on GNOME 50; fix installed (extension and binary 0.12.2, daemon
    active). Blocked: Shell on Wayland loads the new extension.js only at next login,
    which ends this session; after login one flip must log one journal line'
  steps: []
  evidence:
  - path: ".scratch/smoke/sw/log.txt"
    sha256: 9699972a20bc815612b7fc9aa05f64acdba133767fe98e0dde3cf292ee3ad9e7
    bytes: 2724
    kind: log
  - path: ".scratch/smoke/sw/steps.txt"
    sha256: fd2ecd25291d9404f2e64da636044d8fcf6f4a99bbc9273125c53899de9ebe08
    bytes: 460
    kind: file
  runner:
    kind: agent
    harness: claude-code
    harness_version: 2.1.330
    model: claude-opus-5-5
    effort:
  runtime:
    os:
      id: ubuntu
      name: Ubuntu
      version: '26.04'
    kernel: 7.0.0-43-generic
    arch: x86_64
    platform: x86_64-linux-gnu
    container: false
    wsl: false
    tz: PST
    locale: en_US.UTF-8
    ruby:
      engine: ruby
      version: 3.3.8
      platform: x86_64-linux-gnu
    polispec:
      version: 0.59.2
      edition: standard
      commit: 25aea6fd344f87e0598533e8293776af685b8b32
      dirty: false
      install: pinned
    rplugin: 1.20.6
    enforce: enforce
  target:
    commit: 23ddf4fc15b22cd432c8549bedc0b5e2cbf0f44e
    branch: main
    dirty: false
    project:
    env:
    policy_digest:
  prev: 2c14debccf0a60038d22eca55a5519357b97a2b71b03d7e2c2be5fc0e7bccb56
  digest: 2e001a1a2ce1fcfd217525468dae31109d94194cc106ca99e96017e2601587c9
- id: smk_b05d114a62d3
  recorded_at: '2026-11-23T08:24:59Z'
  proves:
    feature: panel-position
  verdict: partial
  summary: 'Daemon 0.13.0 on a private bus: position defaults right, set center and
    left via CLI, GetState carries panel_position, config.yml line rewritten after
    a dated backup, middle refused with exit 1, check prints it, restart keeps it.
    Not observed: the GNOME extension moving the dots, which loads only at next login'
  steps: []
  evidence:
  - path: ".scratch/smoke/pos/out.txt"
    sha256: 3d32c845d31e1b5fed198ecc1056c6792003ef9c518ff9f45c969dd237fb234a
    bytes: 652
    kind: log
  - path: ".scratch/smoke/pos/daemon.log"
    sha256: a57b6aca77b201a0ece6331322a5c09bddb9cf24c211d9a78ef605d06699fbbb
    bytes: 404
    kind: log
  - path: ".scratch/smoke/pos/run.sh"
    sha256: 60c8d262b54a4c361894243c388a3e3f3ca3cb53829adbb3d86c94568cb81d00
    bytes: 1117
    kind: file
  runner:
    kind: agent
    harness: claude-code
    harness_version: 2.1.330
    model: claude-opus-5-5
    effort:
  runtime:
    os:
      id: ubuntu
      name: Ubuntu
      version: '26.04'
    kernel: 7.0.0-43-generic
    arch: x86_64
    platform: x86_64-linux-gnu
    container: false
    wsl: false
    tz: PST
    locale: en_US.UTF-8
    ruby:
      engine: ruby
      version: 3.3.8
      platform: x86_64-linux-gnu
    polispec:
      version: 0.59.2
      edition: standard
      commit: 25aea6fd344f87e0598533e8293776af685b8b32
      dirty: false
      install: pinned
    rplugin: 1.20.6
    enforce: enforce
  target:
    commit: dfb1e9602b672349028d86010286cd78cd4f19c5
    branch: main
    dirty: true
    project:
    env:
    policy_digest:
  prev: 2e001a1a2ce1fcfd217525468dae31109d94194cc106ca99e96017e2601587c9
  digest: 1e1adf1046aa24fd7c6274300ff46dd18d0f07486adc01d8e1925f7bcfa3551c
```
