module Format

export canonical_json

function quoted(value::AbstractString)
    escaped = replace(value, '\\' => "\\\\", '"' => "\\\"", '\n' => "\\n",
                      '\r' => "\\r", '\t' => "\\t")
    string('"', escaped, '"')
end

function emit(io, value)
    if value isa NamedTuple
        print(io, '{')
        for (index, (key, item)) in enumerate(pairs(value))
            index == 1 || print(io, ',')
            print(io, quoted(string(key)), ':')
            emit(io, item)
        end
        print(io, '}')
    elseif value isa AbstractDict
        print(io, '{')
        keys_sorted = sort!(collect(keys(value)); by = string)
        for (index, key) in enumerate(keys_sorted)
            index == 1 || print(io, ',')
            print(io, quoted(string(key)), ':')
            emit(io, value[key])
        end
        print(io, '}')
    elseif value isa AbstractVector || value isa Tuple
        print(io, '[')
        for (index, item) in enumerate(value)
            index == 1 || print(io, ',')
            emit(io, item)
        end
        print(io, ']')
    elseif value isa AbstractString || value isa Symbol
        print(io, quoted(string(value)))
    elseif value isa Bool
        print(io, value ? "true" : "false")
    elseif value isa Integer
        print(io, value)
    elseif value isa AbstractFloat
        isfinite(value) || error("non-finite value cannot enter frozen JSON")
        print(io, repr(value))
    elseif isnothing(value)
        print(io, "null")
    else
        error("unsupported frozen JSON value $(typeof(value))")
    end
end

function canonical_json(value)
    io = IOBuffer()
    emit(io, value)
    print(io, '\n')
    String(take!(io))
end

end
