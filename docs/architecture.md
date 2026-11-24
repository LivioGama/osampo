# Architecture

Generated from the LikeC4 model [`docs/architecture/osampo.c4`](architecture/osampo.c4) by `tools/architecture.sh`. Do not edit by hand.

*Généré depuis le modèle LikeC4 par `tools/architecture.sh` : ne pas modifier à la main.*

## Context

```mermaid
graph LR
  User(["You"])
  Claude["Claude Code"]
  Board["Osampo board"]
  Pc["Linux PC"]
  User -. "pets, throws, walks with" .-> Board
  User -.-> Pc
  Claude -. "runs on each event" .-> Pc
  Board -. "events (walk, reminder answered)" .-> Pc
  Pc -.-> Board
  classDef primary fill:#3b82f6,stroke:#2563eb,color:#eff6ff
  class Claude,Board,Pc primary
  linkStyle 0,1,2,3,4 stroke:#8D8D8D,stroke-dasharray:8 4
```

## Inside the board and the PC

```mermaid
graph LR
  User(["You"])
  Claude["Claude Code"]
  subgraph Board["Osampo board"]
    Board_Firmware["Puppy firmware"]
    Board_Ui["Puppy UI"]
    Board_Driver["Board driver"]
    Board_LinkB["Link library"]
  end
  subgraph Pc["Linux PC"]
    Pc_Settings["osampo-reglages"]
    Pc_Hook["claude_wait.sh"]
    Pc_Simulator["Desktop simulator"]
    Pc_Config[("osampo.toml")]
    Pc_Daemon["osampo-daemon"]
    Pc_TrayApp["osampo-pc"]
    Pc_LinkS["Socket link"]
  end
  User -. "pets, throws, walks with" .-> Board_Firmware
  Board_Firmware -. "shows" .-> Board_Ui
  Board_Firmware -. "uses" .-> Board_Driver
  Board_Firmware -. "reads and writes lines" .-> Board_LinkB
  User -. "sees the dog, answers reminders" .-> Pc_TrayApp
  User -. "tunes" .-> Pc_Settings
  Claude -. "runs on each event" .-> Pc_Hook
  Board_LinkB -. "events (walk, reminder answered)" .-> Pc_Daemon
  Pc_Daemon -.-> Board_LinkB
  Pc_Daemon -. "state, events" .-> Pc_TrayApp
  Pc_Daemon -. "state lines" .-> Pc_LinkS
  Pc_Config -. "read and watched" .-> Pc_Daemon
  Pc_Hook -. "waiting / not waiting" .-> Pc_Daemon
  Pc_Settings -. "edits" .-> Pc_Config
  Pc_Simulator -. "uses" .-> Pc_LinkS
  classDef primary fill:#3b82f6,stroke:#2563eb,color:#eff6ff
  class Claude,Board_Firmware,Board_Ui,Board_Driver,Board_LinkB,Pc_Settings,Pc_Hook,Pc_Simulator,Pc_Config,Pc_Daemon,Pc_TrayApp,Pc_LinkS primary
  linkStyle 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14 stroke:#8D8D8D,stroke-dasharray:8 4
```

## The daemon

```mermaid
graph LR
  PcHook["claude_wait.sh"]
  PcConfig[("osampo.toml")]
  subgraph PcDaemon["osampo-daemon"]
    PcDaemon_Sources["Sources"]
    PcDaemon_Reminders["Reminders and state"]
    PcDaemon_Emitters["Emitters"]
  end
  PcTrayApp["osampo-pc"]
  PcLinkS["Socket link"]
  subgraph Board["Osampo board"]
    Board_LinkB["Link library"]
  end
  PcHook -. "waiting / not waiting" .-> PcDaemon_Sources
  PcDaemon_Sources -. "values" .-> PcDaemon_Emitters
  PcDaemon_Reminders -. "events" .-> PcDaemon_Emitters
  PcDaemon_Emitters -. "state, events" .-> PcTrayApp
  PcDaemon_Emitters -. "state lines" .-> PcLinkS
  PcDaemon_Emitters -.-> Board_LinkB
  Board_LinkB -. "events (walk, reminder answered)" .-> PcDaemon_Emitters
  PcConfig -. "read and watched" .-> PcDaemon
  classDef primary fill:#3b82f6,stroke:#2563eb,color:#eff6ff
  class PcHook,PcConfig,PcDaemon_Sources,PcDaemon_Reminders,PcDaemon_Emitters,PcTrayApp,PcLinkS,Board_LinkB primary
  linkStyle 0,1,2,3,4,5,6,7 stroke:#8D8D8D,stroke-dasharray:8 4
```

## When Claude Code waits

```mermaid
graph LR
  Claude["Claude Code"]
  PcHook["claude_wait.sh"]
  PcDaemonSources["Sources"]
  PcDaemonEmitters["Emitters"]
  BoardLinkB["Link library"]
  BoardFirmware["Puppy firmware"]
  User(["You"])
  PcTrayApp["osampo-pc"]
  Claude -. "Notification / Stop hook" .-> PcHook
  PcHook -. "writes 1" .-> PcDaemonSources
  PcDaemonSources -. "claude_wait=1" .-> PcDaemonEmitters
  PcDaemonEmitters -. "state line, by Bluetooth or USB" .-> BoardLinkB
  BoardLinkB -. "claude_wait=1" .-> BoardFirmware
  BoardFirmware -. "puppy sits and scratches the screen" .-> User
  PcDaemonEmitters -. "claude_wait=1" .-> PcTrayApp
  PcTrayApp -. "paw on the tray dog" .-> User
  classDef primary fill:#3b82f6,stroke:#2563eb,color:#eff6ff
  class Claude,PcHook,PcDaemonSources,PcDaemonEmitters,BoardLinkB,BoardFirmware,PcTrayApp primary
  linkStyle 0,1,2,3,4,5,6,7 stroke:#8D8D8D,stroke-dasharray:8 4
```
