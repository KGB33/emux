local emux = {}

emux.l = {
    envFile = function(path, variable)
        return __emux_env_file(path, variable)
    end,
    files = function(glob)
        return __emux_files(glob)
    end,
    regex = function(target, pattern)
        return __emux_regex(target, pattern)
    end,
}

emux.o = {
    randPort = { __kind = "random_port" },
}

return emux
