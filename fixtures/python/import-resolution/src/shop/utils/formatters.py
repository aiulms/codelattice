def format_price(amount: float, currency: str = "USD") -> str:
    return f"{currency} {amount:.2f}"
