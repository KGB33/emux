local cfg = {
  ["api-port"] = {
    locate = {
      emux.l.envFile(".env", "PORT"),
      emux.l.regex(emux.l.files("environment.local.json"), "4327"),
    },
    override = emux.o.randPort,
  },
}

return cfg
