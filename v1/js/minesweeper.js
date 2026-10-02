( function(){
	var gGame = {

		'gameOn' : false,
		'iMinesCount' : 0,
		'iMaxGridCoordinateX' : 0,
		'iMaxGridCoordinateY' : 0,
		'lstGrid' : [],
		'iFlagsCount' : 0,
		'lstPlacedMinesLocation' : [],
		'gameLevel' : 1,
		'iUnopenedCellCount' :{
			'totalCells' : 0,
			'iFlag' : false
		},
		'timer' : {
			'minutes' : 0,
			'seconds' : 0,
			'timeout' : 0
		} 
	};


	//Main Functions

	var createGrid = function( iGridSizeRow, iGridSizeCol ){

		var lstGridCoordinates = [];
		var divGrid = document.getElementById( 'dGrid' );
		divGrid.innerText = '';

		gGame['lstGrid'] = new Array( iGridSizeRow );
		for ( var x = 0; x < iGridSizeRow; x++ ) {
				
			var divRow = document.createElement( 'div' );
			gGame['lstGrid'][x] = new Array( iGridSizeCol );

			for ( var y = 0; y < iGridSizeCol; y++ ) {
				
				var button = createButton(x, y);
				gGame['lstGrid'][x][y] = button;
				divRow.appendChild( button );
				
				lstGridCoordinates.push( button );				
			}

			divGrid.appendChild( divRow );
		}
		//console.log( 'Grid setup done' );
		return lstGridCoordinates;
	};

	var plantMines = function( lstGridCoordinates ){

		gGame['lstPlacedMinesLocation'] = new Array();

		for( var iCount = 0; iCount < gGame['iMinesCount']; iCount++ ){

			var iRandomNum = getRandomNum( lstGridCoordinates.length );
			var iMineLocation = lstGridCoordinates[iRandomNum];
			
			//Plant a mine
			gGame['lstGrid'][iMineLocation.x][iMineLocation.y].hasValue = -1;
			
			//Add the location to the list
			gGame['lstPlacedMinesLocation'].push( iMineLocation );

			//splice the randomly chosen index for unique random number generation
			lstGridCoordinates.splice( iRandomNum, 1 );
		}
		//console.log( 'Planted Mines' );	
	};

	var setNumbersAroundMines = function(){
		var lstPatterns = new Array( [0, -1], [0, 1], [-1, 0], [1, 0], [-1, -1], [-1, 1], [1, -1], [1, 1] );
		var iLength = lstPatterns.length;

		for( var x = 0; x < gGame['iMaxGridCoordinateX']; x++ ){

			for( var y = 0; y < gGame['iMaxGridCoordinateY']; y++ ){

				var iCell = gGame['lstGrid'][x][y];

				//console.log( iCell.x + ' ' + iCell.y);

				var iMinesAround = 0;
				for( var iLocation = 0; iLocation < iLength; iLocation++ ){
					var iX = lstPatterns[iLocation][0] + iCell.x;
					var iY = lstPatterns[iLocation][1] + iCell.y;

					if( isValidCell( iX, iY ) ){
						var button = gGame['lstGrid'][iX][iY];
				
						if( button.hasValue === -1 ){
							//console.log( 'Mine at: ' +iX + ' ' + iY );
							iMinesAround += 1;
							//console.log( 'Mines count added' ); 
						}
					}
				}

				if( iMinesAround !== 0  && iCell.hasValue !== -1 ){
					iCell.hasValue = iMinesAround;
				}
			}
		}
	};

	var revealCells = function( object ){

		var lstReveal = new Array();
		var lstPatterns = new Array( [0, -1], [0, 1], [-1, 0], [1, 0], [-1, -1], [-1, 1], [1, -1], [1, 1] );

		lstReveal.push( object );
		while( lstReveal.length != 0 ){

			var button = lstReveal.pop();
		

			for( var i = 0; i < lstPatterns.length; i++ ){

				var iX = lstPatterns[i][0] + button.x;
				var iY = lstPatterns[i][1] + button.y;

		
				if( isValidCell(iX, iY ) && button.hasValue !== -1  && button.isVisible === false ){
				
					var nxtBlankButton = gGame['lstGrid'][iX][iY];

					if( nxtBlankButton.hasValue >= 1 && nxtBlankButton.isFlag === false ){
						nxtBlankButton.isVisible = true;
						nxtBlankButton.innerText = nxtBlankButton.hasValue;
						if( nxtBlankButton.hasValue === 1 )
							nxtBlankButton.style.color = 'purple';
						else
							nxtBlankButton.style.color = 'green';
						nxtBlankButton.style.background = 'grey';
					}
					else{

						lstReveal.push( nxtBlankButton );
					}
				}

			}

			button.isVisible = true;
			if( button.isFlag === true ){
				button.innerHTML = null;
				button.isFlag = false;
				var dFlag = document.getElementById( 'flag' );
				gGame['iFlagsCount'] -= 1;
				dFlag.innerHTML = '<img src="media/images/flags/f12.png" />' + '  ' + gGame['iFlagsCount'];

			}
			button.style.background = 'grey';
		}
	};

	var gameTerminate = function( iFlag = -2 ){

		stopTimer();
		gGame['gameOn'] = false;
		var dStartBtn = document.getElementById( 'btnStartGame' );
		dStartBtn.innerHTML = '<h2>' + 'Play Again' + '</h2>';
		dStartBtn.style.visibility = 'visible';
		gGame['iUnopenedCellCount']['iFlag'] = false;

		if( iFlag === -1 ){

			//console.log('isMine' + '\n' + 'Game Over :(' );
			showMines( -1 );//send an image path.
			//button.style.background = 'yellow';// change this to coloring only the outline of the clicked mine button
			setText('Game Over :(');
			
		}
		else if( iFlag === 0 ){
			showMines( 0 );
			//button.style.background = 'yellow';//same as above
			setText('Game Won :)');
		}
		//console.clear();
	};

	//Event Functions

	var eventLeftAction = function( event ){
		//console.log( 'Left Click');	

		var button = getNode( event.target, 'BUTTON' );


		if( ! gGame['gameOn'] || button.isVisible === true ){
			//console.log( 'gameOn is disabled or button is visible');
			return;
		}
		else if( button.isFlag ){
			//console.log('exiting left click function, flag is set ');
			return;
		}
		
		if( gGame['iUnopenedCellCount']['iFlag'] === false && unopenedCells() === 0 ){
			gGame['timer']['timeout'] = setInterval( startTimer, 1000 );
			var dStartBtn = document.getElementById( 'btnStartGame' );
			dStartBtn.style.visibility = 'visible';
			dStartBtn.innerHTML = '<h2>' + 'Restart Game' + '</h2>';
			gGame['iUnopenedCellCount']['iFlag'] = true;
		}

		operations( button );		
	};

	var eventRightAction = function( event ){
		//console.log( 'Right Click');

		event.preventDefault();
		
		var button = getNode( event.target, 'BUTTON' );
		var dFlag = document.getElementById( 'flag' );

		if( ! gGame['gameOn']  || button.isVisible === true ){
			//console.log( 'gameOn is disabled');
			return;
		}
		else if( gGame['iUnopenedCellCount']['iFlag'] === false && unopenedCells() === 0 ){
			gGame['timer']['timeout'] = setInterval( startTimer, 1000 );
			var dStartBtn = document.getElementById( 'btnStartGame' );
			dStartBtn.style.visibility = 'visible';
			gGame['iUnopenedCellCount']['iFlag'] = true;
			dStartBtn.innerHTML = '<h2>' + 'Restart Game' + '</h2>';
		}

		if( ! button.isFlag ){
			//console.log( 'flag set');
			button.isFlag = true;
			//button.isVisible = false;
			//button.style.background = 'cyan';
			gGame['iFlagsCount'] += 1;
			button.innerHTML = '<img src="media/images/flags/f4.png" />';
			dFlag.innerHTML = '<img src="media/images/flags/f12.png" />' + '  ' + gGame['iFlagsCount'];
		}
		else{
			button.isFlag = false;
			gGame['iFlagsCount'] -= 1;
			//button.isVisible = true;
			button.style.background = null;
			button.innerHTML = null;
			dFlag.innerHTML = '<img src="media/images/flags/f12.png" />' + '  ' + gGame['iFlagsCount'];
			//console.log( 'flag unset');
		}
	};

	//Utility Functions

	var operations = function( button ){

		button.oncontextmenu = function(){ event.preventDefault(); };

		if( button.hasValue === -1 ){
			gameTerminate( -1 );
		}
		else{

			if( button.hasValue === 0 ){
				revealCells( button );
			}
			else if( button.hasValue >= 1 ){
				button.isVisible = true;
				button.innerText = button.hasValue;
				if( button.hasValue === 1 )
					button.style.color = 'purple';
				else
					button.style.color = 'green';
				
				button.style.background = 'grey';
			}
			
			var unopened = Math.abs( (gGame['iUnopenedCellCount']['totalCells']) - unopenedCells() );
			//console.log('unopened: ' + unopened );
			if (unopened === gGame['iMinesCount']){
				//console.log('Game Won');
				gameTerminate( 0 );
			}
		}
	};

	var createButton = function( iX, iY ){
		
		var button = document.createElement( 'Button' );
		button.x = iX;
		button.y = iY;
		button.hasValue = 0;
		button.isVisible = false;
		button.isFlag = false;

		//Left click event
		button.onclick = function( event ){ 
			eventLeftAction( event );	 
		};

		// Right click event
		button.oncontextmenu = function( event ){
			event.preventDefault();
			eventRightAction( event );
		};
		return button;
	};

	var getRandomNum = function( max ){
		//Reference from MDN

		var iMin = 0;
		var iMax = Math.floor( max );

		return Math.floor( Math.random() * (iMax - iMin)) + iMin;
	};

	var setText = function( strInfo, init = 0 ){

		if( init == 1 ){

			gGame['timer']['minutes'] = 0;
			gGame['timer']['seconds'] = 0;
			gGame['timer']['timeout'] = 0;

			var dMine = document.getElementById( 'mine' );
			dMine.innerHTML = '<img src="media/images/mines/m2.png" />' + '  ' + gGame['iMinesCount'];

			var dFlag = document.getElementById( 'flag' );
			dFlag.innerHTML = '<img src="media/images/flags/f12.png" />' + '  ' + gGame['iFlagsCount'];

			var dTimer = document.getElementById( 'timer' );
			dTimer.innerText = gGame['timer']['minutes'] + ':' + gGame['timer']['seconds'];
		}

		var dInfo = document.getElementById( 'info' );
		dInfo.innerText = '';
		dInfo.innerText = strInfo;
	};

	var setMineCount = function( row, col, level ){

		//console.log( '--->iLevel: ' + level );
		var iMinimumGridCells = 25;
		var iMines = 0;

		if( level === 1 ){
			iMines = ( 2 * ((row * col) / iMinimumGridCells));
		}
		else if( level === 2 ){
			iMines = ( 4 * ((row * col) / iMinimumGridCells));
		}
		else if( level === 3 ){
			iMines = ( 6 * ((row * col) / iMinimumGridCells));
		}
		else{
			setText( 'Please Choose a valid level');
		}

		return Math.ceil( iMines );
	};

	var startTimer = function(){
		
		var timer = document.getElementById( 'timer' );	
		timer.innerText = null;
		

		if( gGame['timer']['minutes'] === 10 ){
			setText( 'Time Out!');
			gameTerminate();
			return;
		}

		if( gGame['timer']['seconds'] === 59 ){
			gGame['timer']['seconds'] = 0;
			gGame['timer']['minutes'] += 1;
			
			timer.innerText = gGame['timer']['minutes'] + ':' + gGame['timer']['seconds'];
		}
		else{
			gGame['timer']['seconds'] += 1;
			timer.innerText = gGame['timer']['minutes'] + ':' + gGame['timer']['seconds'];
		}
	};

	var stopTimer = function(){
		gGame['timer']['minutes'] = 0;
		gGame['timer']['seconds'] = 0;
		clearInterval( gGame['timer']['timeout'] );
	};

	var isValidCell = function( x, y ){

		try{

			if( (x >= 0 && x <= gGame['iMaxGridCoordinateX']) &&
				(y >= 0 && y <= gGame['iMaxGridCoordinateY']) && 
				( gGame['lstGrid'][x][y] !== undefined) ){

				return true;
			}	
		}
		catch( TypeError ){
			//console.log( 'TypeError: ' + x + ' ' + y);
		}

		return false;
	};

	var showMines = function( iFlag = null ){
		// Display all the mines
		
		var iMinesListLength = gGame['lstPlacedMinesLocation'].length;
		for ( var iMineLocation = 0; iMineLocation < iMinesListLength; iMineLocation++ ) {
			
			var iMine = gGame['lstPlacedMinesLocation'][iMineLocation];
			var button = gGame['lstGrid'][iMine.x][iMine.y];
			//button.isVisible = true;

			if( button.isFlag === true ){
				button.innerHTML = null;
				button.isFlag = false;
				var dFlag = document.getElementById( 'flag' );
				gGame['iFlagsCount'] -= 1;
				dFlag.innerHTML = '<img src="media/images/flags/f12.png" />' + '  ' + gGame['iFlagsCount'];

			}
			if( iFlag === -1 ){
				button.style.background = 'red'; //game over
			}
			else if( iFlag === 0 ){
				button.style.background = 'blue'; //game won
			}
			else{
				button.style.background = 'green';//testing purpose
			}

			button.innerHTML = '<img src="media/images/mines/m2.png" />';
		}	
	};

	var getNode = function ( target, nodeName ){
		while( target.nodeName !== nodeName )
			target = target.parentNode;

		return target;
	};

	var unopenedCells = function(){
		
		var iCount = 0;
		gGame['iUnopenedCellCount']['iFlag'] = true;
		
		for( var i = 0; i < gGame['iMaxGridCoordinateX']; i++ ){

			for( var j = 0; j < gGame['iMaxGridCoordinateY']; j++ ){
				var obj = gGame['lstGrid'][i][j].isVisible; //|| 'false';
				if(  obj === true ){
					iCount += 1;	
				}
			}
		}
		//console.log( 'Visible: ' + iCount );
		return iCount;
	};

	var setupGame = function( iLevel ){

		var lstGridCoordinates = [];
		var iGridSizeCols = 0;
		var iGridSizeRows = 0;
		var iLevel = gGame['gameLevel'];

		//console.log( '-->iLevel: ' + iLevel );

		gGame['gameOn'] = false;
		gGame['totalCells'] = 0;
		gGame['iUnopenedCellCount']['iFlag'] = false;
		gGame['iFlagsCount'] = 0;
		gGame['iMaxGridCoordinateX'] = 0;
		gGame['iMaxGridCoordinateY'] = 0;
		gGame['lstGrid'] = [];
		gGame['lstPlacedMinesLocation'] = [];
		
		stopTimer();
		
		if( iLevel === 1 ){
			iGridSizeCols = 7;
			iGridSizeRows = 7;
		}
		else if( iLevel === 2 ){
			iGridSizeCols = 9;
			iGridSizeRows = 9;
		}
		else if( iLevel === 3 ){
			iGridSizeCols = 11;
			iGridSizeRows = 11;
		}

		gGame['iMaxGridCoordinateX'] = iGridSizeRows;
		gGame['iMaxGridCoordinateY'] = iGridSizeCols;
		gGame['iUnopenedCellCount']['totalCells'] = iGridSizeRows * iGridSizeCols;
		
		gGame['iMinesCount'] = setMineCount( iGridSizeRows, iGridSizeCols, iLevel );
		//console.log( 'mines: ' + gGame['iMinesCount']);

		lstGridCoordinates = createGrid( iGridSizeRows, iGridSizeCols );
		plantMines( lstGridCoordinates );
		setNumbersAroundMines();
	};

	//Init Function

	var selectedLevel = document.getElementById( 'levelList' );
	selectedLevel.onchange = function(){
		gGame['gameLevel'] = parseInt( selectedLevel.options[selectedLevel.selectedIndex].value );
		init();
	};

	var init = function(){
		
		setupGame();	
		setText( 'Let the game begin !', 1 );
		dStartBtn.style.visibility = 'hidden';

		gGame['gameOn'] = true;
	};

	var dStartBtn = document.getElementById( 'btnStartGame' );
	dStartBtn.oncontextmenu = function( event ){ event.preventDefault(); };

	dStartBtn.onclick = function(){
		
		init();
		stopTimer();
	};

	dStartBtn.click();
})();
