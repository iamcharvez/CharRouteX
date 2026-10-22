# Windows helper service

The helper is a separate, narrowly scoped executable. The desktop launches it with Windows `runas`, so every manual network mutation receives explicit UAC authorization. It accepts only a versioned JSON operation document and constructs fixed Windows utility argument arrays from validated typed values—never a shell string.

Before applying, the desktop embeds the observed configuration as the rollback backup. The helper applies the requested configuration, writes status, and waits for a confirmation file. If confirmation does not arrive within 30–300 seconds, it restores the backup automatically.

