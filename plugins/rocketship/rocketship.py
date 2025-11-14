# Example wasm plugin that appends a rocketship emoji to every email that is sent 

import wit_world  # pyright: ignore[reportMissingImports]
from wit_world.exports import Host as HostInterface # pyright: ignore[reportMissingImports]
from componentize_py_types import Err  # pyright: ignore[reportMissingImports]

def handle(e: Exception) -> Err[str]:
    message = str(e)
    if message == "":
        return Err(f"{type(e).__name__}")
    else:
        return Err(f"{type(e).__name__}: {message}")


class WitWorld(wit_world.WitWorld):
    def rocketship(self, statement: str) -> str:
        return statement + "🚀"


class Host(HostInterface):
    def foo(self, x: int) -> int:
        return x + 1
    
    def bar(self, s: str) -> str:
        return s + "🚀"
    
    def baz(self) -> str:
        return "🚀"