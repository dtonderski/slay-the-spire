# Curl Up action ordering

`CurlUpPower.onAttacked` marks itself triggered immediately, then appends its
state change, block gain, and power removal to the bottom of the game's action
queue. Consequently, every contiguous hit already queued by one card resolves
before Curl Up grants block. A later copied card is a separate queued play and
does encounter that block.

The simulator should consume the Curl Up power on the first qualifying hit but
return its block gain as an internal follow-up action. Existing follow-up queue
ordering then keeps the block behind contiguous hits while preserving the
boundary between an original card and a copied play.
