"""
Configuration manager for ezMIgrations.
Loads and validates configuration from YAML file.
"""
from pathlib import Path
from typing import Dict, Any, Optional
import yaml


class ConfigManager:
    """Manages application configuration."""
    
    DEFAULT_CONFIG = {
        "ef_core": {
            "add_command": "dotnet ef migrations add {migration_name}",
            "update_command": "dotnet ef database update {migration_name}",
            "remove_command": "dotnet ef migrations remove"
        },
        "squashed_migration": {
            "name": "SquashedMigration",
            "namespace": "",
            "backup_migrations": True,
            "backup_directory": "../migrations_backup"
        },
        "options": {
            "sort_migrations": True,
            "confirm_deletion": True,
            "rollback_database": True,
            "dry_run": False
        }
    }
    
    def __init__(self, config_path: Optional[Path] = None) -> None:
        """
        Initialize configuration manager.
        
        Args:
            config_path: Path to config file. If None, uses default config.
        """
        self.config: Dict[str, Any] = self.DEFAULT_CONFIG.copy()
        
        if config_path and config_path.exists():
            self.load_config(config_path)
    
    def load_config(self, config_path: Path) -> None:
        """Load configuration from YAML file."""
        try:
            with config_path.open('r') as f:
                user_config = yaml.safe_load(f)
                if user_config:
                    self._merge_config(user_config)
        except Exception as e:
            print(f"Warning: Could not load config from {config_path}: {e}")
            print("Using default configuration.")
    
    def _merge_config(self, user_config: Dict[str, Any]) -> None:
        """Merge user config with default config."""
        for key, value in user_config.items():
            if key in self.config and isinstance(value, dict):
                self.config[key].update(value)
            else:
                self.config[key] = value
    
    def get(self, *keys: str, default: Any = None) -> Any:
        """
        Get configuration value by nested keys.
        
        Example:
            config.get("ef_core", "add_command")
        """
        value = self.config
        for key in keys:
            if isinstance(value, dict) and key in value:
                value = value[key]
            else:
                return default
        return value
    
    def get_add_command(self, migration_name: str) -> str:
        """Get the EF Core add migration command with the migration name."""
        template = self.get("ef_core", "add_command")
        return template.format(migration_name=migration_name)
    
    def get_update_command(self, migration_name: str) -> str:
        """Get the EF Core update database command with the migration name."""
        template = self.get("ef_core", "update_command")
        return template.format(migration_name=migration_name)
    
    def get_remove_command(self) -> str:
        """Get the EF Core remove migration command."""
        return self.get("ef_core", "remove_command")
    
    @property
    def squashed_migration_name(self) -> str:
        """Get the name for the squashed migration."""
        return self.get("squashed_migration", "name", default="SquashedMigration")
    
    @property
    def backup_migrations(self) -> bool:
        """Whether to backup migrations before deleting."""
        return self.get("squashed_migration", "backup_migrations", default=True)
    
    @property
    def backup_directory(self) -> str:
        """Directory to store migration backups."""
        return self.get("squashed_migration", "backup_directory", default="../migrations_backup")
    
    @property
    def dry_run(self) -> bool:
        """Whether running in dry-run mode."""
        return self.get("options", "dry_run", default=False)
    
    @property
    def confirm_deletion(self) -> bool:
        """Whether to prompt for confirmation before deleting migrations."""
        return self.get("options", "confirm_deletion", default=True)
    
    @property
    def rollback_database(self) -> bool:
        """Whether to rollback database before squashing."""
        return self.get("options", "rollback_database", default=True)

