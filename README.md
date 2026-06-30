# Environment Multiplexer 

Problem: You are ~~working~~ vibing on the same project in multiple worktrees,
but you can't run multiple instances of it. All the API servers connect to the
same database, and the client hard coded the port it expects the API to run on. 


`emux` provides a way to modify each worktree s.t. it has a unique environment, and ~~you~~ your agent can work on them simultaneously.


# How It Works

Each project configuration gets one or more `locators` and an `overrider`. 

For example, to override the port the API server runs on:

```fennel
(local cfg
  {:api-port
   {:locate [(emux.l.envFile ".env" "PORT")
             (emux.l.jsonFile "environment.local.json" ".apiPort")]
    :base "4327"
    :override emux.o.port}})

cfg
```

Each entry declares where the value lives (`locate`), its original value
(`base`), and how to replace it (`override`). `emux run config.fnl` writes a
deterministic per-worktree port to every location; re-running is a no-op.
`emux restore config.fnl` writes each `base` value back. Because the override
is a pure function of the worktree path and the values come from the config,
emux needs no state file.
