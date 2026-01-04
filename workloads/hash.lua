local key_gen = bench.key(1000, "uniform", "hash_key")
local field_gen = bench.key(100, "zipfian", "hash_field")
local value_gen = bench.value(32)
function generate()
    local key = key_gen()
    local field = field_gen()
    local value = value_gen()
    return { "HSET", key, field, value }
end