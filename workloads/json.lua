local user_id = bench.key(100000, "uniform", "user_id")
local data = bench.value(64)
local age = bench.rand(100)
function generate()
    key = user_id()
    value = {
        Item = {
            user_id = {N = tostring(key)},
            data = {S = data()},
            age = {N = tostring(age())},
        },
        TableName = "hello",
    }
    json_value = json.encode(value)
    return { "SET", key, json_value }
end