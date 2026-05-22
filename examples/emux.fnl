(local cfg
  {:api-port
   {:locate [(emux.l.envFile "api/.env" "PORT")
             (-> (emux.l.files "client/assets/environment.local.json")
                 (emux.l.regex "8001"))]
    :override emux.o.randPort}})

cfg
