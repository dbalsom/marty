/*

    Machine.rs
    This module defines all the parts that make up the virtual computer.
    This module also contains the main run() method that makes the CPU execute instructions and
    run devices for a given time slice.

*/
use log;

use std::{
    rc::Rc,
    cell::{Cell, RefCell}, collections::VecDeque
};

use crate::{
    bus::BusInterface,
    cga::{self, CGACard},
    cpu::{CpuType, Cpu, Flag, CpuError},
    dma::{self, DMAControllerStringState},
    fdc::{self, FloppyController},
    hdc::{self, HardDiskController},
    floppy_manager::{FloppyManager},
    vhd_manager::{VHDManager},
    io::{IoHandler, IoBusInterface},
    pit::{self, PitStringState},
    pic::{self, PicStringState},
    ppi::{self, PpiStringState},
    rom_manager::RomManager,
};

pub const NUM_FLOPPIES: u32 = 2;
pub const NUM_HDDS: u32 = 2;

pub const MAX_MEMORY_ADDRESS: usize = 0xFFFFF;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum MachineType {
    IBM_PC_5150,
    IBM_XT_5160
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum VideoType {
    MDA,
    CGA,
    EGA,
    VGA
}

#[derive(Copy, Clone, Debug)]
pub enum ExecutionState {
    Paused,
    BreakpointHit,
    Running,
}

pub struct ExecutionControl {
    state: ExecutionState,
    do_step: Cell<bool>,
    do_run: Cell<bool>,
    do_reset: Cell<bool>
}

impl ExecutionControl {
    pub fn new() -> Self {
        Self { 
            state: ExecutionState::Paused,
            do_step: Cell::new(false), 
            do_run: Cell::new(false), 
            do_reset: Cell::new(false)
        }
    }

    pub fn set_state(&mut self, state: ExecutionState) {
        self.state = state
    }

    pub fn get_state(&self) -> ExecutionState {
        self.state
    }

    pub fn do_step(&mut self) {
        self.do_step.set(true);
    }

    pub fn do_run(&mut self) {
        // Run does nothing unless paused or at bp
        match self.state {
            ExecutionState::Paused => {
                self.do_run.set(true);
                self.state = ExecutionState::Running;
            }
            ExecutionState::BreakpointHit => {
                // Step out of breakpoint status into paused status
                self.do_run.set(true);
                self.state = ExecutionState::Running;
            }
            _ => {}
        }        
    }

    pub fn do_reset(&mut self) {
        self.do_reset.set(true)
    }
}
pub struct Machine {
    machine_type: MachineType,
    video_type: VideoType,
    rom_manager: RomManager,
    floppy_manager: FloppyManager,
    bus: BusInterface,
    io_bus: IoBusInterface,
    cpu: Cpu,
    dma_controller: Rc<RefCell<dma::DMAController>>,
    pit: Rc<RefCell<pit::Pit>>,
    pic: Rc<RefCell<pic::Pic>>,
    ppi: Rc<RefCell<ppi::Ppi>>,
    cga: Rc<RefCell<cga::CGACard>>,
    fdc: Rc<RefCell<FloppyController>>,
    hdc: Rc<RefCell<HardDiskController>>,
    kb_buf: VecDeque<u8>,
    error: bool,
    error_str: String,
    cpu_cycles: u64,
}

impl Machine {
    pub fn new(
        machine_type: MachineType,
        video_type: VideoType,
        rom_manager: RomManager,
        floppy_manager: FloppyManager,
        ) -> Machine {

        let mut bus = BusInterface::new();
        let mut io_bus = IoBusInterface::new();
        
        let mut cpu = Cpu::new(CpuType::Cpu8186, 4);
        cpu.reset();        

        // Attach IO Device handlers

        // Intel 8259 Programmable Interrupt Controller
        let mut pic = Rc::new(RefCell::new(pic::Pic::new()));
        io_bus.register_port_handler(pic::PIC_COMMAND_PORT, IoHandler::new(pic.clone()));
        io_bus.register_port_handler(pic::PIC_DATA_PORT, IoHandler::new(pic.clone()));

        // Intel 8255 Programmable Peripheral Interface
        // PPI Needs to know machine_type as DIP switches and thus PPI behavior are different 
        // for PC vs XT
        let mut ppi = Rc::new(RefCell::new(ppi::Ppi::new(machine_type, video_type)));
        io_bus.register_port_handler(ppi::PPI_PORT_A, IoHandler::new(ppi.clone()));
        io_bus.register_port_handler(ppi::PPI_PORT_B, IoHandler::new(ppi.clone()));
        io_bus.register_port_handler(ppi::PPI_PORT_C, IoHandler::new(ppi.clone()));
        io_bus.register_port_handler(ppi::PPI_COMMAND_PORT, IoHandler::new(ppi.clone()));
        
        // Intel 8253 Programmable Interval Timer
        // Ports 0x40,41,42 Data ports, 0x43 Control port
        let mut pit = Rc::new(RefCell::new(pit::ProgrammableIntervalTimer::new()));
        io_bus.register_port_handler(pit::PIT_COMMAND_REGISTER, IoHandler::new(pit.clone()));
        io_bus.register_port_handler(pit::PIT_CHANNEL_0_DATA_PORT, IoHandler::new(pit.clone()));
        io_bus.register_port_handler(pit::PIT_CHANNEL_1_DATA_PORT, IoHandler::new(pit.clone()));
        io_bus.register_port_handler(pit::PIT_CHANNEL_2_DATA_PORT, IoHandler::new(pit.clone()));

        // DMA Controller: 
        // Intel 8237 DMA Controller
        let mut dma = Rc::new(RefCell::new(dma::DMAController::new()));

        io_bus.register_port_handler(dma::DMA_CHANNEL_0_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_0_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_1_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_1_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_2_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_2_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_3_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_3_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_COMMAND_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_WRITE_REQ_REGISTER, IoHandler::new(dma.clone()));

        io_bus.register_port_handler(dma::DMA_CHANNEL_MASK_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_MODE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CLEAR_FLIPFLOP, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_MASTER_CLEAR, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CLEAR_MASK_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_WRITE_MASK_REGISTER, IoHandler::new(dma.clone()));

        io_bus.register_port_handler(dma::DMA_CHANNEL_0_PAGE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_1_PAGE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_2_PAGE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_3_PAGE_REGISTER, IoHandler::new(dma.clone()));

        // Floppy Controller:
        let mut fdc = Rc::new(RefCell::new(fdc::FloppyController::new()));
        io_bus.register_port_handler(fdc::FDC_DIGITAL_OUTPUT_REGISTER, IoHandler::new(fdc.clone()));
        io_bus.register_port_handler(fdc::FDC_STATUS_REGISTER, IoHandler::new(fdc.clone()));
        io_bus.register_port_handler(fdc::FDC_DATA_REGISTER, IoHandler::new(fdc.clone()));

        // Hard Disk Controller:  (Only functions if the required rom is loaded)
        let mut hdc = Rc::new(RefCell::new(hdc::HardDiskController::new(dma.clone(), hdc::DRIVE_TYPE2_DIP)));
        io_bus.register_port_handler(hdc::HDC_DATA_REGISTER, IoHandler::new(hdc.clone()));
        io_bus.register_port_handler(hdc::HDC_STATUS_REGISTER, IoHandler::new(hdc.clone()));
        io_bus.register_port_handler(hdc::HDC_READ_DIP_REGISTER, IoHandler::new(hdc.clone()));
        io_bus.register_port_handler(hdc::HDC_WRITE_MASK_REGISTER, IoHandler::new(hdc.clone()));

        // CGA card:
        let mut cga = Rc::new(RefCell::new(cga::CGACard::new()));
        io_bus.register_port_handler(cga::CRTC_REGISTER_SELECT, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CRTC_REGISTER, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CGA_MODE_CONTROL_REGISTER, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CGA_COLOR_CONTROL_REGISTER, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CGA_STATUS_REGISTER, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CGA_LIGHTPEN_REGISTER, IoHandler::new(cga.clone()));

        // Load BIOS ROM images
        rom_manager.copy_into_memory(&mut bus);

        // Set entry point for ROM (mostly used for diagnostic ROMs that don't have a FAR JUMP reset vector)
        let rom_entry_point = rom_manager.get_entrypoint();
        cpu.set_reset_address(rom_entry_point.0, rom_entry_point.1);
        cpu.reset_address();

        Machine {
            machine_type,
            video_type,
            rom_manager,
            floppy_manager,
            bus: bus,
            io_bus: io_bus,
            cpu: cpu,
            dma_controller: dma,
            pit: pit,
            pic: pic,
            ppi: ppi,
            cga: cga,
            fdc: fdc,
            hdc: hdc,
            kb_buf: VecDeque::new(),
            error: false,
            error_str: String::new(),
            cpu_cycles: 0
        }
    }

    pub fn bus(&self) -> &BusInterface {
        &self.bus
    }

    pub fn mut_bus(&mut self) -> &mut BusInterface {
        &mut self.bus
    }

    pub fn cga(&self) -> Rc<RefCell<CGACard>> {
        self.cga.clone()
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn fdc(&self) -> Rc<RefCell<FloppyController>> {
        self.fdc.clone()
    }

    pub fn hdc(&self) -> Rc<RefCell<HardDiskController>> {
        self.hdc.clone()
    }

    pub fn floppy_manager(&self) -> &FloppyManager {
        &self.floppy_manager
    }

    pub fn cpu_cycles(&self) -> u64 {
        self.cpu_cycles
    }

    pub fn pit_cycles(&self) -> u64 {
        self.pit.borrow().get_cycles()
    }

    pub fn pit_state(&self) -> PitStringState {
        let pit = self.pit.borrow();
        let pit_data = pit.get_string_repr();
        pit_data
    }

    pub fn pic_state(&self) -> PicStringState {
        let pic = self.pic.borrow();
        pic.get_string_state()
    }

    pub fn ppi_state(&self) -> PpiStringState {
        let pic = self.ppi.borrow();
        pic.get_string_state()
    }

    pub fn dma_state(&self) -> DMAControllerStringState {
        let dma = self.dma_controller.borrow();
        dma.get_string_state()
    }

    pub fn get_error_str(&self) -> Option<&str> {
        match self.error {
            true => Some(&self.error_str),
            false => None
        }
    }

    pub fn key_press(&mut self, code: u8) {

        self.kb_buf.push_back(code);
    }

    pub fn key_release(&mut self, code: u8 ) {
        // HO Bit set converts a scancode into its 'release' code
        self.kb_buf.push_back(code | 0x80);
    }

    pub fn reset(&mut self) {
        self.cpu.reset();

        // Clear RAM
        self.bus.reset();

        // Reload BIOS ROM images
        self.rom_manager.copy_into_memory(&mut self.bus);

        // Re-install ROM patches if any
        //self.rom_manager.install_patches(&mut self.bus);

        // Reset devices
        self.pit.borrow_mut().reset();
        self.pic.borrow_mut().reset();
    }
    
    pub fn run(&mut self, cycle_target: u32, exec_control: &mut ExecutionControl, breakpoint: u32) {

        let mut kb_event_processed = false;

        // Was reset requested?
        if exec_control.do_reset.get() {
            self.reset();
            exec_control.do_reset.set(false);
            return
        }
    
        let mut ignore_breakpoint = false;
        let cycle_target_adj = match exec_control.state {
            ExecutionState::Paused => {
                match exec_control.do_step.get() {
                    true => {
                        // Reset step flag
                        exec_control.do_step.set(false);
                        ignore_breakpoint = true;
                        // Execute 1 cycle
                        1
                    },
                    false => return
                }
            }
            ExecutionState::Running => cycle_target,
            ExecutionState::BreakpointHit => cycle_target
        };

        let mut cycles_elapsed = 0;

        while cycles_elapsed < cycle_target_adj {

            let fake_cycles = 7;

            if self.cpu.is_error() == false {

                let flat_address = self.cpu.get_flat_address();

                // Check for immediate breakpoint
                if (flat_address == breakpoint) && breakpoint != 0 && !ignore_breakpoint {

                    return
                }

                // Match checkpoints
                if let Some(cp) = self.rom_manager.get_checkpoint(flat_address) {
                    log::trace!("ROM CHECKPOINT: {}", cp);
                }

                // Check for patching checkpoint & install patches
                if self.rom_manager.is_patch_checkpoint(flat_address) {
                    log::trace!("ROM PATCH CHECKPOINT: Installing ROM patches");
                    self.rom_manager.install_patches(&mut self.bus);
                }

                match self.cpu.step(&mut self.bus, &mut self.io_bus) {
                    Ok(()) => {
                    },
                    Err(err) => {
                        self.error = true;
                        self.error_str = format!("{}", err);
                        log::error!("CPU Error: {}\n{}", err, self.cpu.dump_instruction_history());
                    } 
                }

                // Check for hardware interrupts if Interrupt Flag is set and not in wait cycle
                if self.cpu.interrupts_enabled() {

                    let mut pic = self.pic.borrow_mut();
                    if pic.query_interrupt_line() {
                        match pic.get_interrupt_vector() {
                            Some(irq) =>  self.cpu.do_hw_interrupt(&mut self.bus, irq),
                            None => {}
                        }
                    }
                }

                // Process a keyboard event once per frame.
                // A reasonably fast typist can generate two events in a single 16ms frame, and to the virtual cpu
                // they then appear to happen instantenously. The PPI has no buffer, so one scancode gets lost. 
                // 
                // If we limit keyboard events to once per frame, this avoids this problem. I'm a reasonably
                // fast typist and this method seems to work fine.
                if self.kb_buf.len() > 0 && !kb_event_processed {

                    let kb_byte = self.kb_buf.pop_front().unwrap();

                    self.ppi.borrow_mut().send_keyboard(kb_byte);
                    self.pic.borrow_mut().request_interrupt(1);
                    kb_event_processed = true;
                }

                // Run devices
                
                self.dma_controller.borrow_mut().run(&mut self.io_bus);

                // PIT needs PIC to issue timer interrupts, DMA to do DRAM refresh
                self.pit.borrow_mut().run(
                    &mut self.io_bus,
                    &mut self.bus,
                    &mut self.pic.borrow_mut(),
                    &mut self.dma_controller.borrow_mut(),
                    fake_cycles);

                self.cga.borrow_mut().run(&mut self.io_bus, 7);
                self.ppi.borrow_mut().run(&mut self.pic.borrow_mut(), 7);
                
                // FDC needs PIC to issue controller interrupts, DMA to request DMA transfers, and Memory Bus to read/write to via DMA
                self.fdc.borrow_mut().run(
                    &mut self.pic.borrow_mut(),
                    &mut self.dma_controller.borrow_mut(),
                    &mut self.bus,
                    fake_cycles);

                // HDC needs PIC to issue controller interrupts, DMA to request DMA stransfers, and Memory Bus to read/write to via DMA                    
                self.hdc.borrow_mut().run(
                    &mut self.pic.borrow_mut(),
                    &mut self.dma_controller.borrow_mut(),
                    &mut self.bus,
                    fake_cycles);                
            }
            // Eventually we want to return per-instruction cycle counts, emulate the effect of PIQ, DMA, wait states, all
            // that good stuff. For now during initial development we're going to assume an average instruction cost of 8** 7
            // even cycles keeps the BIOS PIT test from working!
            cycles_elapsed += fake_cycles;
            self.cpu_cycles += fake_cycles as u64;
        }
    }
}