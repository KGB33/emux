(cfg :api-port
  (locate
    (env-file "api/.env" :PORT)
    (-> 
      (file "client/assets/environment.local.json")
      (regex :8001)))
  (override random-port))
