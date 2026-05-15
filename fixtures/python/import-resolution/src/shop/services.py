class OrderService:
    def create(self, item: str, quantity: int) -> str:
        return f"order:{item}:{quantity}"

    def cancel(self, order_id: str) -> bool:
        return True
