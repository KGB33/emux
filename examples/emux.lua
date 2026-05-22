local cfg = {
  ["api-port"] = {
    locate = {
      emux.l.envFile(".env", "PORT"),
      emux.l.regex(emux.l.files("examples/environment.local.json"), "8001"),
    },
    override = emux.o.randPort,
  },
}

return cfg
