local cfg = {
  ["api-port"] = {
    locate = {
      env_file("api/.env", "port"),
      regex(files("client/assets/environment.local.json"), "8001"),
    },
    override = int.random,
  },
}

return cfg
