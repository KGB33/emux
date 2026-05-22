local emux = {}

emux.envFile = function(path, variable)
    return __emux_env_file(path, variable)
end

emux.files = function(glob)
    return __emux_files(glob)
end

emux.regex = function(target, pattern)
    return __emux_regex(target, pattern)
end

emux.int = {
    random = { __kind = "random_port" }
}

return emux
