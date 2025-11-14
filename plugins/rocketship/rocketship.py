# Example wasm plugin that appends a rocketship emoji to every email that is sent 

import bindings.wit_world as wit_world
import bindings.wit_world.imports.termail_host as termail_host

class WitWorld(wit_world.WitWorld):
    """
    Rocketship plugin - appends a rocket emoji to emails before sending
    """

    def on_notify(self, invocation_id: str, event: str) -> bool:
        """
        Called by termail when an event occurs (e.g., before_send)
        """
        print(f"[Rocketship Plugin] Received event: {event} with invocation: {invocation_id}")
        
        # We could call back to the host if needed:
        # result = termail_host.invoke(invocation_id, event)
        # print(f"Host response: {result}")
        
        # Return True to indicate we handled the event
        return True
