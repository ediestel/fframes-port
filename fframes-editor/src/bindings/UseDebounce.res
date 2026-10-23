@module("use-debounce")
external useDebounce: ('a, ~ms: int) => ('a, unit => unit) = "useDebounce"

type useDebounceOption = {"maxWait": int}

@module("use-debounce")
external useDebounceWithOption: ('a, ~ms: int, useDebounceOption) => ('a, unit => unit) =
  "useDebounce"

let useThrottle = (val, ~ms) => {
  useDebounceWithOption(
    val,
    ~ms,
    {
      "maxWait": ms,
    },
  )
}
