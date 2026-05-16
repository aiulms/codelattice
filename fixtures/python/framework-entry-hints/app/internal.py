def helper_function():
    """Internal helper — should NOT be a framework entry hint"""
    return 42

def _private_helper():
    """Private internal — should definitely NOT be flagged"""
    return 0
