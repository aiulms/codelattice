# Simulated click CLI — decorated functions are CLI entry candidates
import click

@click.command()
def sync_command():
    """Sync data from remote — CLI command handler"""
    pass

def helper_only():
    """Not a CLI command — just internal logic"""
    pass
