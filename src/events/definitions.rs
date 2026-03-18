use crate::define_operator_events;

define_operator_events! {
    pub enum OperatorEvent {
        // ───────────────────────────────── EBO Lifecycle ────────────────────────────────
        EboCreated                  => { message: "New EboServer",                       level: Info,    k8s: true },
        EboDeleting                 => { message: "Start deleting EBO",                  level: Info,    k8s: true },
        BackupScheduleChange        => { message: "Backup schedule changed",             level: Info,    k8s: true },
        ImageUpgrade                => { message: "Image/version changed",               level: Info,    k8s: true },
        BackupStarted               => { message: "Backup started",                      level: Info,    k8s: true },
        BackupOngoing               => { message: "Backup ongoing",                      level: Info,    k8s: true },
        BackupPerformed             => { message: "Backup performed",                    level: Info,    k8s: true },
        BackupSucceeded             => { message: "Backup succeeded",                    level: Info,    k8s: true },
        BackupFailed                => { message: "Backup failed",                       level: Error,   k8s: true },
        OutOfSpaceBackup            => { message: "Out of space for backup",             level: Error,   k8s: true },
        UpgradeFailed               => { message: "Upgrade failed",                      level: Error,   k8s: true },
        UpgradeDisabled             => { message: "Upgrading is disabled",               level: Warning, k8s: true },
        VolumeResizeRequest         => { message: "Volume resize requested",             level: Info,    k8s: true },
        PasswordSecretCreated       => { message: "Password secret created",             level: Info,    k8s: true },
        TemporaryPasswordActive     => { message: "Temporary password active",           level: Info,    k8s: true },
        TemporaryPasswordExpired    => { message: "Temporary password expired",          level: Warning, k8s: true },
        TemporaryPasswordReset      => { message: "Temporary password reset",            level: Info,    k8s: true },
        PasswordResetInitiated      => { message: "Password reset initiated",            level: Info,    k8s: true },
        PasswordResetJobCreated     => { message: "Password reset job created",          level: Info,    k8s: true },
        PasswordResetJobSucceeded   => { message: "Password reset job finished",         level: Info,    k8s: true },
        RestoreInitiated            => { message: "Restore initiated",                   level: Info,    k8s: true },

        // ───────────────────────────────── Pod Lifecycle ────────────────────────────────
        PodCreated                  => { message: "Pod created",                         level: Info,    k8s: true },
        PodReady                    => { message: "Pod running",                         level: Info,    k8s: true },
        PodDeleted                  => { message: "Pod deleted",                         level: Info,    k8s: true },
        PodStopping                 => { message: "Pod stopping",                        level: Info,    k8s: true },
        PodStopped                  => { message: "Pod stopped",                         level: Info,    k8s: true },
        SpecDriftDetected           => { message: "Pod spec drift detected",             level: Warning, k8s: true },
        CrashLoopDetected           => { message: "Pod crash loop detected",             level: Error,   k8s: true },
        CrashLoopRecovered          => { message: "Pod crash loop recovered",            level: Info,    k8s: true },
        // ─────────────────────────────── Upgrade Job Lifecycle ──────────────────────────
        UpgradeJobCreated           => { message: "Upgrade job created",                 level: Info,    k8s: true },
        UpgradeJobSucceeded         => { message: "Upgrade job succeeded",               level: Info,    k8s: true },
        // ─────────────────────────────── Restore Job Lifecycle ──────────────────────────
        RestoreJobCreated           => { message: "Restore job created",                 level: Info,    k8s: true },
        RestoreJobSucceeded         => { message: "Restore job succeeded",               level: Info,    k8s: true },

        // ─────────────────────────────────── Registration ───────────────────────────────
        Registered                  => { message: "EBO registered",                      level: Info,    k8s: true },
        NotRegistered               => { message: "EBO not registered",                  level: Error,   k8s: true },

        // ───────────────────────────────────── Licensing ────────────────────────────────
        LicensesActivated           => { message: "Licenses activated",                  level: Info,    k8s: true },
        LicensesReturned            => { message: "Licenses returned",                   level: Info,    k8s: true },
        LicensesSeatActivated       => { message: "Licenses seat activated",             level: Info,    k8s: true },
        LicensesSeatReturned        => { message: "Licenses seat returned",              level: Info,    k8s: true },
        LicensesNotActivated        => { message: "License error activating",            level: Error,   k8s: true },
        LicensesNotReturned         => { message: "License error returning",             level: Error,   k8s: true },
        LicensesSeatNotActivated    => { message: "License error seat activating",       level: Error,   k8s: true },
        LicensesSeatNotReturned     => { message: "License error seat returning",        level: Error,   k8s: true },

        // ──────────────────────────────────────── DNS ───────────────────────────────────
        HTTPChallengeCreated        => { message: "Http challenge created",              level: Info,    k8s: true },
        DNSRecordCreated            => { message: "DNS record created",                  level: Info,    k8s: true },
        DNSRecordError              => { message: "DNS record error",                    level: Error,   k8s: true },
        OriginDNSRecordCreated      => { message: "Origin DNS record created",           level: Info,    k8s: true },

        // ─────────────────────────────────────── WAF ────────────────────────────────────
        WAFHostnameExists           => { message: "WAF hostname exists",                 level: Info,    k8s: true },
        WAFHostnameCreated          => { message: "WAF hostnames created",               level: Info,    k8s: true },
        SecurityConfigHostnameAdded => { message: "WAF security config hostname added",  level: Info,    k8s: true },
    }
}
