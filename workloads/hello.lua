local user_id = bench.key(10000, "uniform", "user_id")
local data = bench.value(64)
function generate()
    key = user_id()
    value = data()
    return { "SET", key, value }
end