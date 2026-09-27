package dev.carbonhv.session

enum class SessionInput {
    EscapeHandle,
    SystemHome,
    PowerButton,
    ViewportTouch,
    GuestBack,
    GuestHome,
    GuestOverview,
}

enum class InputOwner {
    Host,
    Guest,
}

object SessionInputPolicy {
    fun ownerOf(input: SessionInput): InputOwner = when (input) {
        SessionInput.EscapeHandle,
        SessionInput.SystemHome,
        SessionInput.PowerButton -> InputOwner.Host
        SessionInput.ViewportTouch,
        SessionInput.GuestBack,
        SessionInput.GuestHome,
        SessionInput.GuestOverview -> InputOwner.Guest
    }
}

