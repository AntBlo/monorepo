use std::marker::PhantomData;

use embedded_hal::digital::v2::OutputPin;
use embedded_sdmmc::{
    sdcard::AcquireOpts, BlockDevice, Directory, File, Mode, SdCard, TimeSource, Timestamp, Volume,
    VolumeIdx, VolumeManager,
};
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::AnyIOPin,
    peripheral::Peripheral,
    spi::{config::Duplex, SpiAnyPins, SpiConfig, SpiDeviceDriver, SpiDriver},
    units::Hertz,
};
use log::error;

const MODEL_FILE_NAME: &str = "model.txt";

pub trait BlockDev
where
    Self: BlockDevice + Send + 'static,
{
}
impl<T: BlockDevice + Send + 'static> BlockDev for T {}

pub struct WrappedReaderWriter<'a, T, D: BlockDevice> {
    volume_manager: &'a mut VolumeManager<D, FakeTimeSource>,
    volume: &'a mut Volume,
    dir: &'a mut Directory,
    file: Option<File>,
    line_buffer: String,
    _phantom: PhantomData<T>,
}

pub struct Writer;
pub struct Reader;

impl<'a, D: BlockDevice> WrappedReaderWriter<'a, Writer, D> {
    pub fn write(&mut self, line: &str) -> Result<(), StorageLineWriterError> {
        let mut num_written = 0;
        while let Some(line) = line.get(num_written..) {
            if line.is_empty() {
                break;
            }

            let mut retries = 0;
            loop {
                match self.volume_manager.write(
                    self.volume,
                    self.file.as_mut().unwrap(),
                    line.as_bytes(),
                ) {
                    Ok(value) => {
                        num_written += value;
                        break;
                    }
                    Err(err) => {
                        if retries >= 1000 {
                            error!("{err:#?}");
                            FreeRtos::delay_ms(10);
                            return Err(StorageLineWriterError::Write);
                        }
                        retries += 1;
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'a, T, D: BlockDevice> Drop for WrappedReaderWriter<'a, T, D> {
    fn drop(&mut self) {
        for _ in 0..3 {
            match self
                .volume_manager
                .close_file(self.volume, self.file.take().unwrap())
            {
                Ok(_) => break,
                Err(err) => error!("{err:#?}"),
            }
        }
    }
}

impl<'a, D: BlockDevice> WrappedReaderWriter<'a, Reader, D> {
    pub fn read(&mut self) -> Result<Option<String>, StorageLineReaderError> {
        let mut buffer = [0u8; 100];

        if self.file.as_mut().unwrap().eof() {
            return Ok(None);
        }

        let index_after_newline = loop {
            if let Some(newline_index) = self.line_buffer.find('\n') {
                break newline_index + 1;
            }

            let num_read = self
                .volume_manager
                .read(self.volume, self.file.as_mut().unwrap(), &mut buffer)
                .map_err(|err| {
                    error!("{err:#?}");
                    StorageLineReaderError::Read
                })?;

            self.line_buffer += core::str::from_utf8(&buffer[..num_read]).map_err(|err| {
                error!("{err:#?}");
                StorageLineReaderError::Utf8Error
            })?;

            if self.file.as_mut().unwrap().eof() {
                return Ok(Some(self.line_buffer.clone()));
            }
        };

        let mut line = self.line_buffer.split_off(index_after_newline);
        core::mem::swap(&mut self.line_buffer, &mut line);

        Ok(Some(line))
    }

    pub fn file_size_in_bytes(&mut self) -> u32 {
        self.file.as_ref().unwrap().length()
    }

    pub fn remaining_bytes_in_file(&mut self) -> u32 {
        self.file.as_ref().unwrap().left()
    }
}

pub struct StorageWrapper<D: BlockDevice> {
    volume_manager: VolumeManager<D, FakeTimeSource>,
    volume: Volume,
    dir: Directory,
}

impl<D: BlockDevice> StorageWrapper<D> {
    pub fn get_writer(&mut self) -> WrappedReaderWriter<Writer, D> {
        self.create_wrapper(Mode::ReadWriteCreate)
    }

    pub fn get_reader(&mut self) -> WrappedReaderWriter<Reader, D> {
        self.create_wrapper(Mode::ReadOnly)
    }

    pub fn delete(&mut self) -> Result<(), StorageDeleteError> {
        self.volume_manager
            .delete_file_in_dir(&self.volume, &self.dir, MODEL_FILE_NAME)
            .map_err(|err| {
                error!("{err:#?}");
                StorageDeleteError::DeleteFileInDir
            })?;
        Ok(())
    }

    fn create_wrapper<T>(&mut self, mode: Mode) -> WrappedReaderWriter<T, D> {
        let file = self
            .volume_manager
            .open_file_in_dir(&mut self.volume, &self.dir, MODEL_FILE_NAME, mode)
            .map_err(|err| {
                error!("{err:#?}");
                err
            })
            .unwrap();

        WrappedReaderWriter {
            volume_manager: &mut self.volume_manager,
            volume: &mut self.volume,
            dir: &mut self.dir,
            file: Some(file),
            line_buffer: String::new(),
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub enum StorageLineReaderError {
    Read,
    Utf8Error,
}

#[derive(Debug)]
pub enum StorageLineWriterError {
    Write,
}

#[derive(Debug)]
pub enum StorageDeleteError {
    GetVolume,
    OpenRootDir,
    DeleteFileInDir,
}

struct FakeTimeSource;

impl TimeSource for FakeTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub fn create_storage(
    spi: impl Peripheral<P = impl SpiAnyPins> + 'static,
    sclk: impl Into<AnyIOPin>,
    sdo: impl Into<AnyIOPin>,
    sdi: impl Into<AnyIOPin>,
    cs: impl OutputPin + 'static,
) -> StorageWrapper<impl BlockDevice> {
    let spi_driver = SpiDriver::new(
        spi,
        sclk.into(),
        sdo.into(),
        Some(sdi.into()),
        &Default::default(),
    )
    .expect("Should create spi_driver");
    let spi_device_driver = SpiDeviceDriver::new(
        spi_driver,
        None::<AnyIOPin>,
        &SpiConfig {
            baudrate: Hertz(15_000_000),
            duplex: Duplex::Full,
            ..Default::default()
        },
    )
    .expect("Should create ");

    let sd_card = SdCard::new_with_options(
        spi_device_driver,
        cs,
        FreeRtos {},
        AcquireOpts { use_crc: true },
    );

    let mut volume_manager = VolumeManager::new(sd_card, FakeTimeSource {});

    let volume = volume_manager
        .get_volume(VolumeIdx(0))
        .map_err(|err| {
            error!("{err:#?}");
            StorageDeleteError::GetVolume
        })
        .unwrap();
    let dir = volume_manager
        .open_root_dir(&volume)
        .map_err(|err| {
            error!("{err:#?}");
            StorageDeleteError::OpenRootDir
        })
        .unwrap();

    StorageWrapper {
        volume_manager,
        volume,
        dir,
    }
}
