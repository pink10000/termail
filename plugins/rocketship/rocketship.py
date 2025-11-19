# Example wasm plugin that appends a rocketship emoji to every email that is sent 

from wit_world import WitWorld
from wit_world.imports import host_api

class WitWorld(WitWorld):
    """
    Rocketship plugin - appends a rocket emoji to emails before sending
    """

    def on_notify(self, invocation_id: str, event: str) -> str:
        """
        Called by termail when an event occurs (e.g., before_send)
        Returns the modified event data
        """
        host_api.call_host(invocation_id, "Rocketship plugin active!")
        # Simply append a rocket emoji and return the modified event
        return event + " 🚀"
