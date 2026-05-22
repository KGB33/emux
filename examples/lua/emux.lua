local cfg = {
  ["api-port"] = {
    locate = {
      emux.envFile("api/.env", "PORT"),
      emux.regex(emux.files("client/assets/environment.local.json"), "8001"),
    },
    override = emux.int.random,
  },
}

return cfg
