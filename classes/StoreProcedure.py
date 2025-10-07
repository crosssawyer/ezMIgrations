from typing import Optional


class StoredProcedure:
    def __init__(self, name: str, up_method: str, down_method: str) -> None:
        self.name: str = name
        self.latest_up_method: str = up_method
        self.oldest_down_method: str = down_method
    
    def update_up_method(self, new_up_method: str) -> None:
        self.latest_up_method = new_up_method
    
    def update_down_method(self, new_down_method: str) -> None:
        self.oldest_down_method = new_down_method