# HTTP weather plugin

`http/weather` requests the configured `url`, replacing `{city}` with the encoded City input
(default Kyiv). The text endpoint must return `temperature,description`, for example
`22.5,Sunny`, with temperature in degrees Celsius.

Temp (output 4) is `quantity.temperature`; its plain numeric value is canonical Celsius.
Description (output 5) remains text. Success and Failure are execution outputs; there are no
other numeric outputs. Quantity formatting follows the project display rules, so the editor or
To text can display Fahrenheit without changing the endpoint contract.

A new request cancels unfinished work. The host restricts HTTP domains to the manifest's allowed
list. Existing saved nodes with a confirmed Number Temp pin need pin confirmation after updating.
