/******************************************************************************
* Copyright (C) 2010 - 2021 Xilinx, Inc.  All rights reserved.
* SPDX-License-Identifier: MIT
******************************************************************************/

/*****************************************************************************/
/**
 *
 * @file xaxidma_example_sg_intr.c
 *
 * This file demonstrates how to use the xaxidma driver on the Xilinx AXI
 * DMA core (AXIDMA) to transfer packets in interrupt mode when the AXIDMA
 * core is configured in Scatter Gather Mode
 *
 * We show how to do multiple packets transfers, as well as how to do multiple
 * BDs per packet transfers.
 *
 * This code assumes a loopback hardware widget is connected to the AXI DMA
 * core for data packet loopback.
 *
 * To see the debug print, you need a Uart16550 or uartlite in your system,
 * and please set "-DDEBUG" in your compiler options. You need to rebuild your
 * software executable.
 *
 * Make sure that MEMORY_BASE is defined properly as per the HW system. The
 * h/w system built in Area mode has a maximum DDR memory limit of 64MB. In
 * throughput mode, it is 512MB.  These limits are need to ensured for
 * proper operation of this code.
 *
 *
 * <pre>
 * MODIFICATION HISTORY:
 *
 * Ver   Who  Date     Changes
 * ----- ---- -------- -------------------------------------------------------
 * 1.00a jz   05/18/10 First release
 * 2.00a jz   08/10/10 Second release, added in xaxidma_g.c, xaxidma_sinit.c,
 *		       		   updated tcl file, added xaxidma_porting_guide.h, removed
 *		         	   workaround for endianness
 * 4.00a rkv  02/22/11 Name of the file has been changed for naming consistency
 *		       		   Added interrupt support for Zynq.
 * 5.00a srt  03/05/12 Added Flushing and Invalidation of Caches to fix CRs
 *		       		   648103, 648701.
 *		       		   Added V7 DDR Base Address to fix CR 649405.
 * 6.00a srt  01/24/12 Changed API calls to support MCDMA driver.
 * 7.00a srt  06/18/12 API calls are reverted back for backward compatibility.
 * 7.01a srt  11/02/12 Buffer sizes (Tx and Rx) are modified to meet maximum
 *		       DDR memory limit of the h/w system built with Area mode
 * 7.02a srt  03/01/13 Updated DDR base address for IPI designs (CR 703656).
 * 9.1   adk  01/07/16 Updated DDR base address for Ultrascale (CR 799532) and
 *		       removed the defines for S6/V6.
 * 9.3   ms   01/23/17 Modified xdbg_printf statement in main function to
 *                     ensure that "Successfully ran" and "Failed" strings are
 *                     available in all examples. This is a fix for CR-965028.
 *       ms   04/05/17 Added tabspace for return statements in functions
 *                     for proper documentation while generating doxygen.
 * 9.6   rsp  02/14/18 Support data buffers above 4GB.Use UINTPTR for storing
 *                     and typecasting buffer address(CR-992638).
 * 9.9   rsp  01/21/19 Fix use of #elif check in deriving DDR_BASE_ADDR.
 * 9.10  rsp  09/17/19 Fix cache maintenance ops for source and dest buffer.
 * </pre>
 *
 * ***************************************************************************
 */
/***************************** Include Files *********************************/
#include "xaxidma.h"
#include "xaxiethernet.h"
#include "xparameters.h"
// #include "xil_exception.h"
#include "xdebug.h"
#include "xaxiethernet_example.h"

/******************** Constant Definitions **********************************/
/*
 * Device hardware build related constants.
 */

#define AXIETHERNET_DEVICE_ID   XPAR_AXIETHERNET_0_DEVICE_ID
#define DMA_DEV_ID		XPAR_AXI_DMA_0_DEVICE_ID

#define MEM_BASE_ADDR       0x101F0000   //todo

#ifdef XPAR_INTC_0_DEVICE_ID
#define RX_INTR_ID		XPAR_INTC_0_AXIDMA_0_S2MM_INTROUT_VEC_ID
#define TX_INTR_ID		XPAR_INTC_0_AXIDMA_0_MM2S_INTROUT_VEC_ID
#else
#define RX_INTR_ID		XPAR_FABRIC_AXIDMA_0_S2MM_INTROUT_VEC_ID
#define TX_INTR_ID		XPAR_FABRIC_AXIDMA_0_MM2S_INTROUT_VEC_ID
#endif

#define RX_BD_SPACE_BASE	(MEM_BASE_ADDR)
#define RX_BD_SPACE_HIGH	(MEM_BASE_ADDR + 0x0000FFFF)
#define TX_BD_SPACE_BASE	(MEM_BASE_ADDR + 0x00010000)
#define TX_BD_SPACE_HIGH	(MEM_BASE_ADDR + 0x0001FFFF)
#define TX_BUFFER_BASE		(MEM_BASE_ADDR + 0x00100000)
#define RX_BUFFER_BASE		(MEM_BASE_ADDR + 0x00300000)
#define RX_BUFFER_HIGH		(MEM_BASE_ADDR + 0x004FFFFF)

/* Timeout loop counter for reset
 */
#define RESET_TIMEOUT_COUNTER	10000

/*
 * Buffer and Buffer Descriptor related constant definition
 */
#define MAX_PKT_LEN		0x100
#define MARK_UNCACHEABLE        0x701

/*
 * Number of BDs in the transfer example
 * We show how to submit multiple BDs for one transmit.
 * The receive side gets one completion per transfer.
 */
#define NUMBER_OF_BDS_PER_PKT		12
#define NUMBER_OF_PKTS_TO_TRANSFER 	11
#define NUMBER_OF_BDS_TO_TRANSFER	(NUMBER_OF_PKTS_TO_TRANSFER * \
						NUMBER_OF_BDS_PER_PKT)

/* The interrupt coalescing threshold and delay timer threshold
 * Valid range is 1 to 255
 *
 * We set the coalescing threshold to be the total number of packets.
 * The receive side will only get one completion interrupt for this example.
 */
#define COALESCING_COUNT		NUMBER_OF_PKTS_TO_TRANSFER
#define DELAY_TIMER_COUNT		100

#define AXIETHERNET_LOOPBACK_SPEED  100 /* 100Mb/s for Mii */
#define AXIETHERNET_LOOPBACK_SPEED_1G   1000    /* 1000Mb/s for GMii */

/**************************** Type Definitions *******************************/


/***************** Macros (Inline Functions) Definitions *********************/


/************************** Function Prototypes ******************************/
static int CheckData(int Length, u8 StartValue);
static void TxCallBack(XAxiDma_BdRing * TxRingPtr);
static void TxIntrHandler(void *Callback);
static void RxCallBack(XAxiDma_BdRing * RxRingPtr);
static void RxIntrHandler(void *Callback);



static int SetupIntrSystem(XAxiDma * AxiDmaPtr, u16 TxIntrId, u16 RxIntrId);
static void DisableIntrSystem(u16 TxIntrId, u16 RxIntrId);

static int RxSetup(XAxiDma * AxiDmaInstPtr);
static int TxSetup(XAxiDma * AxiDmaInstPtr);
static int SendPacket(XAxiDma * AxiDmaInstPtr);

/************************** Variable Definitions *****************************/
/*
 * Device instance definitions
 */
static XAxiDma AxiDma;

static u8 LocalMacAddr[6] = {0x00, 0x0A, 0x35, 0x03, 0x02, 0x03};

/*
 * Flags interrupt handlers use to notify the application context the events.
 */
volatile int TxDone;
volatile int RxDone;
volatile int Error;

/*
 * Buffer for transmit packet. Must be 32-bit aligned to be used by DMA.
 */
static u32 *Packet = (u32 *) TX_BUFFER_BASE;

static int init_mac(XAxiEthernet_Config *MacCfgPtr) {
    int Status;
    int LoopbackSpeed;

    /* Initialize AxiEthernet hardware */
    Status = XAxiEthernet_CfgInitialize(&AxiEthernetInstance, MacCfgPtr, MacCfgPtr->BaseAddress);
    if (Status != 0) {
        xdbg_printf(XDBG_DEBUG_ERROR, "AXI Ethernet initialization failed " << Status << "\n");
        return 1;
    }

    /* Set the MAC  address */
    Status = XAxiEthernet_SetMacAddress(&AxiEthernetInstance, (u8*)LocalMacAddr);
    if (Status != 0) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Error setting MAC address\n");
        return 1;
    }

	/*
	 * Set PHY to loopback, speed depends on phy type.
	 * MII is 100 and all others are 1000.
	 */
	if (XAxiEthernet_GetPhysicalInterface(&AxiEthernetInstance) ==
							XAE_PHY_TYPE_MII) {
		LoopbackSpeed = AXIETHERNET_LOOPBACK_SPEED;
	} else {
		LoopbackSpeed = AXIETHERNET_LOOPBACK_SPEED_1G;
	}
	Status = AxiEthernetUtilEnterLoopback(&AxiEthernetInstance,
							LoopbackSpeed);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_ERROR, "Error setting the PHY loopback");
		return XST_FAILURE;
	}

	/*
	 * Set PHY<-->MAC data clock
	 */
	Status = XAxiEthernet_SetOperatingSpeed(&AxiEthernetInstance,
					(u16)LoopbackSpeed);
	if (Status != XST_SUCCESS) {
		return XST_FAILURE;
	}

    xdbg_printf(XDBG_DEBUG_GENERAL, "MAC initialized, waiting 2sec...\n");

	/*
	 * Setting the operating speed of the MAC needs a delay.  There
	 * doesn't seem to be register to poll, so please consider this
	 * during your application design.
	 */
	AxiEthernetUtilPhyDelay(2);

    xdbg_printf(XDBG_DEBUG_GENERAL, "MAC initialization done\n");

    return 0;
}

/*****************************************************************************/
/**
*
* Main function
*
* This function is the main entry of the interrupt test. It does the following:
*	- Set up the output terminal if UART16550 is in the hardware build
*	- Initialize the DMA engine
*	- Set up Tx and Rx channels
*	- Set up the interrupt system for the Tx and Rx interrupts
*	- Submit a transfer
*	- Wait for the transfer to finish
*	- Check transfer status
*	- Disable Tx and Rx interrupts
*	- Print test status and exit
*
* @param	None
*
* @return
*		- XST_SUCCESS if tests pass
*		- XST_FAILURE if fails.
*
* @note		None.
*
******************************************************************************/
int main_example_dma_intr(void)
{
	int Status;
	XAxiDma_Config *Config;
    XAxiEthernet_Config *MacCfgPtr;

	xdbg_printf(XDBG_DEBUG_GENERAL, "\n--- Entering main() --- \n");

	Config = XAxiDma_LookupConfig(DMA_DEV_ID);
	if (!Config) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "No config found for " << DMA_DEV_ID << "\n");

		return XST_FAILURE;
	}

    /* Get the configuration of AxiEthernet hardware */
    MacCfgPtr = XAxiEthernet_LookupConfig(AXIETHERNET_DEVICE_ID);

    /* Check whether AXI DMA is present or not */
    if(MacCfgPtr->AxiDevType != XPAR_AXI_DMA) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Device HW not configured for DMA mode\n");
        return XST_FAILURE;
    }

	xdbg_printf(XDBG_DEBUG_GENERAL, "initializing DMA engine\n");

	/* Initialize DMA engine */
	XAxiDma_CfgInitialize(&AxiDma, Config);

	if(!XAxiDma_HasSg(&AxiDma)) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Device configured as Simple mode\n");
		return XST_FAILURE;
	}

	xdbg_printf(XDBG_DEBUG_GENERAL, "TxSetup\n");

	/* Set up TX/RX channels to be ready to transmit and receive packets */
	Status = TxSetup(&AxiDma);

	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed TX setup\n");
		return XST_FAILURE;
	}

	xdbg_printf(XDBG_DEBUG_GENERAL, "RxSetup\n");

	Status = RxSetup(&AxiDma);
	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed RX setup\n");
		return XST_FAILURE;
	}

	init_mac(MacCfgPtr);

	xdbg_printf(XDBG_DEBUG_GENERAL, "Enable Rx and Tx\n");

    /*
     * Make sure Tx and Rx are enabled
     */
    Status = XAxiEthernet_SetOptions(&AxiEthernetInstance,
                         XAE_RECEIVER_ENABLE_OPTION | XAE_TRANSMITTER_ENABLE_OPTION);
    if (Status != XST_SUCCESS) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Error setting options");
        return XST_FAILURE;
    }

    /*
     * Start the Axi Ethernet and enable its ERROR interrupts
     */
    XAxiEthernet_Start(&AxiEthernetInstance);

	xdbg_printf(XDBG_DEBUG_GENERAL, "Setup interrupts\n");

	/* Set up Interrupt system  */
	Status = SetupIntrSystem(&AxiDma, TX_INTR_ID, RX_INTR_ID);
	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed intr setup\n");
		return XST_FAILURE;
	}

	/* Initialize flags before start transfer test  */
	TxDone = 0;
	RxDone = 0;
	Error = 0;

	xdbg_printf(XDBG_DEBUG_GENERAL, "Sending Packet\n");

	/* Send a packet */
	Status = SendPacket(&AxiDma);
	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed send packet\n");
		return XST_FAILURE;
	}

	xdbg_printf(XDBG_DEBUG_GENERAL, "Waiting until TX and RX done\n");

	XAxiDma_BdRing *TxRingPtr = XAxiDma_GetTxRing(&AxiDma);
	XAxiDma_BdRing *RxRingPtr = XAxiDma_GetRxRing(&AxiDma);
	/*
	 * Wait TX done and RX done
	 */
	while (((TxDone < NUMBER_OF_BDS_TO_TRANSFER) ||
			(RxDone < NUMBER_OF_BDS_TO_TRANSFER)) && !Error) {
		/* NOP */

		RxIntrHandler(RxRingPtr);
		TxIntrHandler(TxRingPtr);

	}

	xdbg_printf(XDBG_DEBUG_GENERAL, "TX and RX done\n");

	if (Error) {
		xdbg_printf(XDBG_DEBUG_GENERAL,
			"Failed test transmit" << (TxDone ? "" : " not") << " done, "
			"receive" << (RxDone ? "" : " not") << " done\n");
		goto Done;

	}else {

		/*
		 * Test finished, check data
		 */
		Status = CheckData(MAX_PKT_LEN * NUMBER_OF_BDS_TO_TRANSFER,
									0xC);
		if (Status != XST_SUCCESS) {

			xdbg_printf(XDBG_DEBUG_GENERAL, "Data check failed\n");

			goto Done;
		}

		xdbg_printf(XDBG_DEBUG_GENERAL, "Successfully ran AXI DMA SG interrupt Example\n");
	}

	xdbg_printf(XDBG_DEBUG_GENERAL, "Disable interrupts\n");

	/* Disable TX and RX Ring interrupts and return success */

	DisableIntrSystem(TX_INTR_ID, RX_INTR_ID);

Done:

	xdbg_printf(XDBG_DEBUG_GENERAL, "--- Exiting main() --- \n");

	if (Status != XST_SUCCESS) {
		return XST_FAILURE;
	}

	return XST_SUCCESS;
}

/*****************************************************************************/
/*
*
* This function checks data buffer after the DMA transfer is finished.
*
* We use the static tx/rx buffers.
*
* @param	Length is the length to check
* @param	StartValue is the starting value of the first byte
*
* @return	- XST_SUCCESS if validation is successful
*		- XST_FAILURE if validation fails.
*
* @note		None.
*
******************************************************************************/
static int CheckData(int Length, u8 StartValue)
{
	u8 *RxPacket;
	int Index = 0;
	u8 Value;

	RxPacket = (u8 *) RX_BUFFER_BASE;
	Value = StartValue;

	/* Invalidate the DestBuffer before receiving the data, in case the
	 * Data Cache is enabled
	 */
	// Xil_DCacheInvalidateRange((UINTPTR)RxPacket, Length);

	for(Index = 0; Index < Length; Index++) {
		if (RxPacket[Index] != Value) {
			xdbg_printf(XDBG_DEBUG_GENERAL,
				"Data error " << Index << ": " << RxPacket[Index] << "/" << Value << "\n");
			return XST_FAILURE;
		}
		Value = (Value + 1) & 0xFF;
	}

	return XST_SUCCESS;
}

/*****************************************************************************/
/*
*
* This is the DMA TX callback function to be called by TX interrupt handler.
* This function handles BDs finished by hardware.
*
* @param	TxRingPtr is a pointer to TX channel of the DMA engine.
*
* @return	None.
*
* @note		None.
*
******************************************************************************/
static void TxCallBack(XAxiDma_BdRing * TxRingPtr)
{
	int BdCount;
	u32 BdSts;
	XAxiDma_Bd *BdPtr;
	XAxiDma_Bd *BdCurPtr;
	int Status;
	int Index;

	/* Get all processed BDs from hardware */
	BdCount = XAxiDma_BdRingFromHw(TxRingPtr, XAXIDMA_ALL_BDS, &BdPtr);

	/* Handle the BDs */
	BdCurPtr = BdPtr;
	for (Index = 0; Index < BdCount; Index++) {

		/*
		 * Check the status in each BD
		 * If error happens, the DMA engine will be halted after this
		 * BD processing stops.
		 */
		BdSts = XAxiDma_BdGetSts(BdCurPtr);
		if ((BdSts & XAXIDMA_BD_STS_ALL_ERR_MASK) ||
		    (!(BdSts & XAXIDMA_BD_STS_COMPLETE_MASK))) {

			Error = 1;
			break;
		}

		/*
		 * Here we don't need to do anything. But if a RTOS is being
		 * used, we may need to free the packet buffer attached to
		 * the processed BD
		 */

		/* Find the next processed BD */
		BdCurPtr = (XAxiDma_Bd *)XAxiDma_BdRingNext(TxRingPtr, BdCurPtr);
	}

	/* Free all processed BDs for future transmission */
	Status = XAxiDma_BdRingFree(TxRingPtr, BdCount, BdPtr);
	if (Status != XST_SUCCESS) {
		Error = 1;
	}

	if(!Error) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Transmitted " << BdCount << " packets\n");
		TxDone += BdCount;
	}
	else {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Error during transmission\n");
	}
}

/*****************************************************************************/
/*
*
* This is the DMA TX Interrupt handler function.
*
* It gets the interrupt status from the hardware, acknowledges it, and if any
* error happens, it resets the hardware. Otherwise, if a completion interrupt
* presents, then it calls the callback function.
*
* @param	Callback is a pointer to TX channel of the DMA engine.
*
* @return	None.
*
* @note		None.
*
******************************************************************************/
static void TxIntrHandler(void *Callback)
{
	XAxiDma_BdRing *TxRingPtr = (XAxiDma_BdRing *) Callback;
	u32 IrqStatus;
	int TimeOut;

	/* Read pending interrupts */
	IrqStatus = XAxiDma_BdRingGetIrq(TxRingPtr);

	xdbg_printf(XDBG_DEBUG_GENERAL, "TxStatus = " << m3::fmt(IrqStatus, "#x") << "\n");

	/* Acknowledge pending interrupts */
	XAxiDma_BdRingAckIrq(TxRingPtr, IrqStatus);

	/* If no interrupt is asserted, we do not do anything
	 */
	if (!(IrqStatus & XAXIDMA_IRQ_ALL_MASK)) {

		return;
	}

	/*
	 * If error interrupt is asserted, raise error flag, reset the
	 * hardware to recover from the error, and return with no further
	 * processing.
	 */
	if ((IrqStatus & XAXIDMA_IRQ_ERROR_MASK)) {

		XAxiDma_BdRingDumpRegs(TxRingPtr);

		Error = 1;

		/*
		 * Reset should never fail for transmit channel
		 */
		XAxiDma_Reset(&AxiDma);

		TimeOut = RESET_TIMEOUT_COUNTER;

		while (TimeOut) {
			if (XAxiDma_ResetIsDone(&AxiDma)) {
				break;
			}

			TimeOut -= 1;
		}

		return;
	}

	/*
	 * If Transmit done interrupt is asserted, call TX call back function
	 * to handle the processed BDs and raise the according flag
	 */
	if ((IrqStatus & (XAXIDMA_IRQ_DELAY_MASK | XAXIDMA_IRQ_IOC_MASK))) {
		TxCallBack(TxRingPtr);
	}
}

/*****************************************************************************/
/*
*
* This is the DMA RX callback function called by the RX interrupt handler.
* This function handles finished BDs by hardware, attaches new buffers to those
* BDs, and give them back to hardware to receive more incoming packets
*
* @param	RxRingPtr is a pointer to RX channel of the DMA engine.
*
* @return	None.
*
* @note		None.
*
******************************************************************************/
static void RxCallBack(XAxiDma_BdRing * RxRingPtr)
{
	int BdCount;
	XAxiDma_Bd *BdPtr;
	XAxiDma_Bd *BdCurPtr;
	u32 BdSts;
	int Index;

	/* Get finished BDs from hardware */
	BdCount = XAxiDma_BdRingFromHw(RxRingPtr, XAXIDMA_ALL_BDS, &BdPtr);

	BdCurPtr = BdPtr;
	for (Index = 0; Index < BdCount; Index++) {

		/*
		 * Check the flags set by the hardware for status
		 * If error happens, processing stops, because the DMA engine
		 * is halted after this BD.
		 */
		BdSts = XAxiDma_BdGetSts(BdCurPtr);
		if ((BdSts & XAXIDMA_BD_STS_ALL_ERR_MASK) ||
		    (!(BdSts & XAXIDMA_BD_STS_COMPLETE_MASK))) {
			Error = 1;
			break;
		}

		/* Find the next processed BD */
		BdCurPtr = (XAxiDma_Bd *)XAxiDma_BdRingNext(RxRingPtr, BdCurPtr);
		RxDone += 1;
		xdbg_printf(XDBG_DEBUG_GENERAL, "Received packet\n");
	}

}

/*****************************************************************************/
/*
*
* This is the DMA RX interrupt handler function
*
* It gets the interrupt status from the hardware, acknowledges it, and if any
* error happens, it resets the hardware. Otherwise, if a completion interrupt
* presents, then it calls the callback function.
*
* @param	Callback is a pointer to RX channel of the DMA engine.
*
* @return	None.
*
* @note		None.
*
******************************************************************************/
static void RxIntrHandler(void *Callback)
{
	XAxiDma_BdRing *RxRingPtr = (XAxiDma_BdRing *) Callback;
	u32 IrqStatus;
	int TimeOut;

	/* Read pending interrupts */
	IrqStatus = XAxiDma_BdRingGetIrq(RxRingPtr);

	xdbg_printf(XDBG_DEBUG_GENERAL, "RxStatus = " << m3::fmt(IrqStatus, "#x") << "\n");

	/* Acknowledge pending interrupts */
	XAxiDma_BdRingAckIrq(RxRingPtr, IrqStatus);

	/*
	 * If no interrupt is asserted, we do not do anything
	 */
	if (!(IrqStatus & XAXIDMA_IRQ_ALL_MASK)) {
		return;
	}

	/*
	 * If error interrupt is asserted, raise error flag, reset the
	 * hardware to recover from the error, and return with no further
	 * processing.
	 */
	if ((IrqStatus & XAXIDMA_IRQ_ERROR_MASK)) {

		XAxiDma_BdRingDumpRegs(RxRingPtr);

		Error = 1;

		/* Reset could fail and hang
		 * NEED a way to handle this or do not call it??
		 */
		XAxiDma_Reset(&AxiDma);

		TimeOut = RESET_TIMEOUT_COUNTER;

		while (TimeOut) {
			if(XAxiDma_ResetIsDone(&AxiDma)) {
				break;
			}

			TimeOut -= 1;
		}

		return;
	}

	/*
	 * If completion interrupt is asserted, call RX call back function
	 * to handle the processed BDs and then raise the according flag.
	 */
	if ((IrqStatus & (XAXIDMA_IRQ_DELAY_MASK | XAXIDMA_IRQ_IOC_MASK))) {
		RxCallBack(RxRingPtr);
	}
}

/*****************************************************************************/
/*
*
* This function setups the interrupt system so interrupts can occur for the
* DMA, it assumes INTC component exists in the hardware system.
*
* @param	IntcInstancePtr is a pointer to the instance of the INTC.
* @param	AxiDmaPtr is a pointer to the instance of the DMA engine
* @param	TxIntrId is the TX channel Interrupt ID.
* @param	RxIntrId is the RX channel Interrupt ID.
*
* @return
*		- XST_SUCCESS if successful,
*		- XST_FAILURE.if not successful
*
* @note		None.
*
******************************************************************************/

static int SetupIntrSystem(XAxiDma * AxiDmaPtr, u16 TxIntrId, u16 RxIntrId)
{
// XAxiDma_BdRing *TxRingPtr = XAxiDma_GetTxRing(AxiDmaPtr);
// XAxiDma_BdRing *RxRingPtr = XAxiDma_GetRxRing(AxiDmaPtr);
// int Status;

// #ifdef XPAR_INTC_0_DEVICE_ID

// 	/* Initialize the interrupt controller and connect the ISRs */
// 	Status = XIntc_Initialize(IntcInstancePtr, INTC_DEVICE_ID);
// 	if (Status != XST_SUCCESS) {

// 		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed init intc\n");
// 		return XST_FAILURE;
// 	}

// 	Status = XIntc_Connect(IntcInstancePtr, TxIntrId,
// 			       (XInterruptHandler) TxIntrHandler, TxRingPtr);
// 	if (Status != XST_SUCCESS) {

// 		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed tx connect intc\n");
// 		return XST_FAILURE;
// 	}

// 	Status = XIntc_Connect(IntcInstancePtr, RxIntrId,
// 			       (XInterruptHandler) RxIntrHandler, RxRingPtr);
// 	if (Status != XST_SUCCESS) {

// 		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed rx connect intc\n");
// 		return XST_FAILURE;
// 	}

// 	/* Start the interrupt controller */
// 	Status = XIntc_Start(IntcInstancePtr, XIN_REAL_MODE);
// 	if (Status != XST_SUCCESS) {

// 		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed to start intc\n");
// 		return XST_FAILURE;
// 	}

// 	XIntc_Enable(IntcInstancePtr, TxIntrId);
// 	XIntc_Enable(IntcInstancePtr, RxIntrId);

// #else

// 	XScuGic_Config *IntcConfig;


// 	/*
// 	 * Initialize the interrupt controller driver so that it is ready to
// 	 * use.
// 	 */
// 	IntcConfig = XScuGic_LookupConfig(INTC_DEVICE_ID);
// 	if (NULL == IntcConfig) {
// 		return XST_FAILURE;
// 	}

// 	Status = XScuGic_CfgInitialize(IntcInstancePtr, IntcConfig,
// 					IntcConfig->CpuBaseAddress);
// 	if (Status != XST_SUCCESS) {
// 		return XST_FAILURE;
// 	}


// 	XScuGic_SetPriorityTriggerType(IntcInstancePtr, TxIntrId, 0xA0, 0x3);

// 	XScuGic_SetPriorityTriggerType(IntcInstancePtr, RxIntrId, 0xA0, 0x3);
// 	/*
// 	 * Connect the device driver handler that will be called when an
// 	 * interrupt for the device occurs, the handler defined above performs
// 	 * the specific interrupt processing for the device.
// 	 */
// 	Status = XScuGic_Connect(IntcInstancePtr, TxIntrId,
// 				(Xil_InterruptHandler)TxIntrHandler,
// 				TxRingPtr);
// 	if (Status != XST_SUCCESS) {
// 		return Status;
// 	}

// 	Status = XScuGic_Connect(IntcInstancePtr, RxIntrId,
// 				(Xil_InterruptHandler)RxIntrHandler,
// 				RxRingPtr);
// 	if (Status != XST_SUCCESS) {
// 		return Status;
// 	}

// 	XScuGic_Enable(IntcInstancePtr, TxIntrId);
// 	XScuGic_Enable(IntcInstancePtr, RxIntrId);
// #endif

// 	/* Enable interrupts from the hardware */

// 	Xil_ExceptionInit();
// 	Xil_ExceptionRegisterHandler(XIL_EXCEPTION_ID_INT,
// 			(Xil_ExceptionHandler)INTC_HANDLER,
// 			(void *)IntcInstancePtr);

// 	Xil_ExceptionEnable();

	return XST_SUCCESS;
}

/*****************************************************************************/
/**
*
* This function disables the interrupts for DMA engine.
*
* @param	IntcInstancePtr is the pointer to the INTC component instance
* @param	TxIntrId is interrupt ID associated w/ DMA TX channel
* @param	RxIntrId is interrupt ID associated w/ DMA RX channel
*
* @return	None.
*
* @note		None.
*
******************************************************************************/
static void DisableIntrSystem(u16 TxIntrId, u16 RxIntrId)
{
// #ifdef XPAR_INTC_0_DEVICE_ID
// 	/* Disconnect the interrupts for the DMA TX and RX channels */
// 	XIntc_Disconnect(IntcInstancePtr, TxIntrId);
// 	XIntc_Disconnect(IntcInstancePtr, RxIntrId);
// #else
// 	XScuGic_Disconnect(IntcInstancePtr, TxIntrId);
// 	XScuGic_Disconnect(IntcInstancePtr, RxIntrId);
// #endif
}

/*****************************************************************************/
/*
*
* This function sets up RX channel of the DMA engine to be ready for packet
* reception
*
* @param	AxiDmaInstPtr is the pointer to the instance of the DMA engine.
*
* @return	- XST_SUCCESS if the setup is successful.
*		- XST_FAILURE if fails.
*
* @note		None.
*
******************************************************************************/
static int RxSetup(XAxiDma * AxiDmaInstPtr)
{
	XAxiDma_BdRing *RxRingPtr;
	int Status;
	XAxiDma_Bd BdTemplate;
	XAxiDma_Bd *BdPtr;
	XAxiDma_Bd *BdCurPtr;
	int BdCount;
	int FreeBdCount;
	UINTPTR RxBufferPtr;
	int Index;

	RxRingPtr = XAxiDma_GetRxRing(&AxiDma);

	/* Disable all RX interrupts before RxBD space setup */
	XAxiDma_BdRingIntDisable(RxRingPtr, XAXIDMA_IRQ_ALL_MASK);

	/* Setup Rx BD space */
	BdCount = XAxiDma_BdRingCntCalc(XAXIDMA_BD_MINIMUM_ALIGNMENT,
				RX_BD_SPACE_HIGH - RX_BD_SPACE_BASE + 1);

	Status = XAxiDma_BdRingCreate(RxRingPtr, RX_BD_SPACE_BASE,
					RX_BD_SPACE_BASE,
					XAXIDMA_BD_MINIMUM_ALIGNMENT, BdCount);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Rx bd create failed with " << Status << "\n");
		return XST_FAILURE;
	}

	/*
	 * Setup a BD template for the Rx channel. Then copy it to every RX BD.
	 */
	XAxiDma_BdClear(&BdTemplate);
	Status = XAxiDma_BdRingClone(RxRingPtr, &BdTemplate);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Rx bd clone failed with " << Status << "\n");
		return XST_FAILURE;
	}

	/* Attach buffers to RxBD ring so we are ready to receive packets */
	FreeBdCount = XAxiDma_BdRingGetFreeCnt(RxRingPtr);

	Status = XAxiDma_BdRingAlloc(RxRingPtr, FreeBdCount, &BdPtr);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Rx bd alloc failed with " << Status << "\n");
		return XST_FAILURE;
	}

	BdCurPtr = BdPtr;
	RxBufferPtr = RX_BUFFER_BASE;

	for (Index = 0; Index < FreeBdCount; Index++) {

		Status = XAxiDma_BdSetBufAddr(BdCurPtr, RxBufferPtr);
		if (Status != XST_SUCCESS) {
			xdbg_printf(XDBG_DEBUG_GENERAL,
				"Rx set buffer addr " << m3::fmt(RxBufferPtr, "#x") <<
				" on BD " << m3::fmt((void*)BdCurPtr, "#x") << " failed " << Status << "\n");
			return XST_FAILURE;
		}

		Status = XAxiDma_BdSetLength(BdCurPtr, MAX_PKT_LEN,
					RxRingPtr->MaxTransferLen);
		if (Status != XST_SUCCESS) {
			xdbg_printf(XDBG_DEBUG_GENERAL,
				"Rx set length " << MAX_PKT_LEN <<
				" on BD " << m3::fmt((void*)BdCurPtr, "#x") << " failed " << Status << "\n");
			return XST_FAILURE;
		}

		/* Receive BDs do not need to set anything for the control
		 * The hardware will set the SOF/EOF bits per stream status
		 */
		XAxiDma_BdSetCtrl(BdCurPtr, 0);

		XAxiDma_BdSetId(BdCurPtr, RxBufferPtr);

		RxBufferPtr += MAX_PKT_LEN;
		BdCurPtr = (XAxiDma_Bd *)XAxiDma_BdRingNext(RxRingPtr, BdCurPtr);
	}

	/*
	 * Set the coalescing threshold, so only one receive interrupt
	 * occurs for this example
	 *
	 * If you would like to have multiple interrupts to happen, change
	 * the COALESCING_COUNT to be a smaller value
	 */
	Status = XAxiDma_BdRingSetCoalesce(RxRingPtr, COALESCING_COUNT,
			DELAY_TIMER_COUNT);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Rx set coalesce failed with " << Status << "\n");
		return XST_FAILURE;
	}

	Status = XAxiDma_BdRingToHw(RxRingPtr, FreeBdCount, BdPtr);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Rx ToHw failed with " << Status << "\n");
		return XST_FAILURE;
	}

	/* Enable all RX interrupts */
	XAxiDma_BdRingIntEnable(RxRingPtr, XAXIDMA_IRQ_ALL_MASK);

	/* Start RX DMA channel */
	Status = XAxiDma_BdRingStart(RxRingPtr);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Rx start BD ring failed with " << Status << "\n");
		return XST_FAILURE;
	}

	return XST_SUCCESS;
}

/*****************************************************************************/
/*
*
* This function sets up the TX channel of a DMA engine to be ready for packet
* transmission.
*
* @param	AxiDmaInstPtr is the pointer to the instance of the DMA engine.
*
* @return	- XST_SUCCESS if the setup is successful.
*		- XST_FAILURE otherwise.
*
* @note		None.
*
******************************************************************************/
static int TxSetup(XAxiDma * AxiDmaInstPtr)
{
	XAxiDma_BdRing *TxRingPtr = XAxiDma_GetTxRing(&AxiDma);
	XAxiDma_Bd BdTemplate;
	int Status;
	u32 BdCount;

	/* Disable all TX interrupts before TxBD space setup */
	XAxiDma_BdRingIntDisable(TxRingPtr, XAXIDMA_IRQ_ALL_MASK);

	/* Setup TxBD space  */
	BdCount = XAxiDma_BdRingCntCalc(XAXIDMA_BD_MINIMUM_ALIGNMENT,
			(UINTPTR)TX_BD_SPACE_HIGH - (UINTPTR)TX_BD_SPACE_BASE + 1);

	Status = XAxiDma_BdRingCreate(TxRingPtr, TX_BD_SPACE_BASE,
				     TX_BD_SPACE_BASE,
				     XAXIDMA_BD_MINIMUM_ALIGNMENT, BdCount);
	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed create BD ring\n");
		return XST_FAILURE;
	}

	/*
	 * Like the RxBD space, we create a template and set all BDs to be the
	 * same as the template. The sender has to set up the BDs as needed.
	 */
	XAxiDma_BdClear(&BdTemplate);
	Status = XAxiDma_BdRingClone(TxRingPtr, &BdTemplate);
	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed clone BDs\n");
		return XST_FAILURE;
	}

	/*
	 * Set the coalescing threshold, so only one transmit interrupt
	 * occurs for this example
	 *
	 * If you would like to have multiple interrupts to happen, change
	 * the COALESCING_COUNT to be a smaller value
	 */
	Status = XAxiDma_BdRingSetCoalesce(TxRingPtr, COALESCING_COUNT,
			DELAY_TIMER_COUNT);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL,
			"Failed set coalescing " << COALESCING_COUNT << "/" << DELAY_TIMER_COUNT << "\n");
		return XST_FAILURE;
	}

	/* Enable all TX interrupts */
	XAxiDma_BdRingIntEnable(TxRingPtr, XAXIDMA_IRQ_ALL_MASK);

	/* Start the TX channel */
	Status = XAxiDma_BdRingStart(TxRingPtr);
	if (Status != XST_SUCCESS) {
		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed bd start\n");
		return XST_FAILURE;
	}

	return XST_SUCCESS;
}

/*****************************************************************************/
/*
*
* This function non-blockingly transmits all packets through the DMA engine.
*
* @param	AxiDmaInstPtr points to the DMA engine instance
*
* @return
* 		- XST_SUCCESS if the DMA accepts all the packets successfully,
* 		- XST_FAILURE if error occurs
*
* @note		None.
*
******************************************************************************/
static int SendPacket(XAxiDma * AxiDmaInstPtr)
{
	XAxiDma_BdRing *TxRingPtr = XAxiDma_GetTxRing(AxiDmaInstPtr);
	u8 *TxPacket;
	u8 Value;
	XAxiDma_Bd *BdPtr, *BdCurPtr;
	int Status;
	int Index, Pkts;
	UINTPTR BufferAddr;

	/*
	 * Each packet is limited to TxRingPtr->MaxTransferLen
	 *
	 * This will not be the case if hardware has store and forward built in
	 */
	if (MAX_PKT_LEN * NUMBER_OF_BDS_PER_PKT >
			TxRingPtr->MaxTransferLen) {
		xdbg_printf(XDBG_DEBUG_GENERAL,
			"Invalid total per packet transfer length for the "
		    "packet " << (MAX_PKT_LEN * NUMBER_OF_BDS_PER_PKT) << "/" << TxRingPtr->MaxTransferLen << "\n");

		return XST_INVALID_PARAM;
	}

	TxPacket = (u8 *) Packet;

	Value = 0xC;

	for(Index = 0; Index < MAX_PKT_LEN * NUMBER_OF_BDS_TO_TRANSFER;
								Index ++) {
		TxPacket[Index] = Value;

		Value = (Value + 1) & 0xFF;
	}

	/* Flush the buffers before the DMA transfer, in case the Data Cache
	 * is enabled
	 */
	// Xil_DCacheFlushRange((UINTPTR)TxPacket, MAX_PKT_LEN *
	// 						NUMBER_OF_BDS_TO_TRANSFER);
	// Xil_DCacheFlushRange((UINTPTR)RX_BUFFER_BASE, MAX_PKT_LEN *
	// 						NUMBER_OF_BDS_TO_TRANSFER);

	Status = XAxiDma_BdRingAlloc(TxRingPtr, NUMBER_OF_BDS_TO_TRANSFER,
								&BdPtr);
	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed bd alloc\n");
		return XST_FAILURE;
	}

	BufferAddr = (UINTPTR)Packet;
	BdCurPtr = BdPtr;

	/*
	 * Set up the BD using the information of the packet to transmit
	 * Each transfer has NUMBER_OF_BDS_PER_PKT BDs
	 */
	for(Index = 0; Index < NUMBER_OF_PKTS_TO_TRANSFER; Index++) {

		for(Pkts = 0; Pkts < NUMBER_OF_BDS_PER_PKT; Pkts++) {
			u32 CrBits = 0;

			Status = XAxiDma_BdSetBufAddr(BdCurPtr, BufferAddr);
			if (Status != XST_SUCCESS) {
				xdbg_printf(XDBG_DEBUG_GENERAL,
					"Tx set buffer addr " << m3::fmt(BufferAddr, "#x") <<
					" on BD " << m3::fmt((void*)BdCurPtr, "#x") << " failed " << Status << "\n");
				return XST_FAILURE;
			}

			Status = XAxiDma_BdSetLength(BdCurPtr, MAX_PKT_LEN,
						TxRingPtr->MaxTransferLen);
			if (Status != XST_SUCCESS) {
				xdbg_printf(XDBG_DEBUG_GENERAL,
					"Tx set length " << MAX_PKT_LEN <<
					" on BD " << m3::fmt((void*)BdCurPtr, "#x") << " failed " << Status << "\n");
				return XST_FAILURE;
			}

			if (Pkts == 0) {
				/* The first BD has SOF set
				 */
				CrBits |= XAXIDMA_BD_CTRL_TXSOF_MASK;

#if (XPAR_AXIDMA_0_SG_INCLUDE_STSCNTRL_STRM == 1)
				/* The first BD has total transfer length set
				 * in the last APP word, this is for the
				 * loopback widget
				 */
				Status = XAxiDma_BdSetAppWord(BdCurPtr,
				    XAXIDMA_LAST_APPWORD,
				    MAX_PKT_LEN * NUMBER_OF_BDS_PER_PKT);

				if (Status != XST_SUCCESS) {
					xdbg_printf(XDBG_DEBUG_GENERAL, "Set app word failed with " << Status << "\n");
				}
#endif
			}

			if(Pkts == (NUMBER_OF_BDS_PER_PKT - 1)) {
				/* The last BD should have EOF and IOC set
				 */
				CrBits |= XAXIDMA_BD_CTRL_TXEOF_MASK;
			}

			XAxiDma_BdSetCtrl(BdCurPtr, CrBits);
			XAxiDma_BdSetId(BdCurPtr, BufferAddr);

			BufferAddr += MAX_PKT_LEN;
			BdCurPtr = (XAxiDma_Bd *)XAxiDma_BdRingNext(TxRingPtr, BdCurPtr);
		}
	}

	/* Give the BD to hardware */
	Status = XAxiDma_BdRingToHw(TxRingPtr, NUMBER_OF_BDS_TO_TRANSFER,
						BdPtr);
	if (Status != XST_SUCCESS) {

		xdbg_printf(XDBG_DEBUG_GENERAL, "Failed to hw, length " << (int)XAxiDma_BdGetLength(BdPtr,
					TxRingPtr->MaxTransferLen) << "\n");

		return XST_FAILURE;
	}

	return XST_SUCCESS;
}
