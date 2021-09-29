def eparam_path(*elms: str) -> str:
    """Convert EParam paths to JSON.

    Args:
        *elms: All identifying elements.

    Returns:
        JSON string.
    """
    start = ""
    end = ""

    for elm in elms:
        start += f"""{{"{elm}": """
        end += "}"
    result = start + "null" + end
    return result
