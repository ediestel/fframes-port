type htmlParserOptions = {xmlMode: bool}
type parseOptions = {htmlparser2: htmlParserOptions}

@module("html-react-parser")
external parseReactElement: (string, parseOptions) => React.element = "default"
