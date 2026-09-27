package dev.carbonhv.domain

enum class HostStatus {
    ReadyAccelerated,
    CpuAcceleratedGraphicsExperimental,
    Unsupported,
}

object AdmissionText {
    fun forStatus(status: HostStatus): String = when (status) {
        HostStatus.ReadyAccelerated -> "Ready for accelerated sessions"
        HostStatus.CpuAcceleratedGraphicsExperimental ->
            "CPU acceleration available; graphics experimental"
        HostStatus.Unsupported -> "This host does not allow full-system VMs"
    }
}

