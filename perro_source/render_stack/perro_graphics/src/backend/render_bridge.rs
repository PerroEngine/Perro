use super::*;

impl RenderBridge for PerroGraphics {
    fn submit(&mut self, command: RenderCommand) {
        self.frame.queue(command);
        // Pending commands already bypass the idle gate. Their post-apply
        // dirty bits decide whether pixels changed; marking redraw here made
        // byte-identical camera extraction force acquire/present forever.
    }

    fn submit_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.frame.pending_commands.extend(commands);
        // See `submit`: pending state itself wakes the draw path.
    }

    fn drain_events(&mut self, out: &mut Vec<RenderEvent>) {
        out.append(&mut self.events);
    }
}
