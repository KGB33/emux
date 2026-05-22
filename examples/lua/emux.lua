local cfg = {
  ["api-port"] = {
    locate = {
      emux.l.envFile("api/.env", "PORT"),
      emux.l.regex(emux.l.files("client/assets/environment.local.json"), "8001"),
    },
    override = emux.o.randPort,
  },
}

return cfg
