use super::*;

impl RenderBridge for PerroGraphics {
    fn submit(&mut self, command: RenderCommand) {
        self.frame.queue(command);
        self.redraw_requested = true;
    }

    fn submit_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.frame.pending_commands.extend(commands);
        self.redraw_requested = true;
    }

    fn drain_events(&mut self, out: &mut Vec<RenderEvent>) {
        out.append(&mut self.events);
    }
}
